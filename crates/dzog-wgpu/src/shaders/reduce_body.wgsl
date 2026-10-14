// Zero-Readback GPU Force, Torque, Fracture & Binaural Probe Reduction Pass.
//
// Each invocation (0..body_count) integrates net radiation pressure force (-grad p),
// Bohmian guidance force (-grad Q), torque (r x F)_z, and peak overpressure |p|
// across a body's bounding disk directly on the GPU, writing compact 16-byte
// GpuBodyReduction records without reading back the full RGBA32Float field texture.

struct SimParams {
    width: u32,
    height: u32,
    dx: f32,
    dy: f32,
    dt: f32,
    inv_dx: f32,
    inv_dy: f32,
    outer_rule: u32,
    beta: f32,
    alpha: f32,
    threshold_decay: f32,
    _pad: f32,
}

struct GpuBodyQuery {
    x: f32,
    y: f32,
    radius: f32,
    yield_pressure: f32,
}

struct GpuBodyReduction {
    fx: f32,
    fy: f32,
    tau_z: f32,
    max_pressure: f32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var state_in: texture_2d<f32>; // RGBA: (p, vx, vy, Q)
@group(0) @binding(2) var<storage, read> queries: array<GpuBodyQuery>;
@group(0) @binding(3) var<storage, read_write> reductions: array<GpuBodyReduction>;

@compute @workgroup_size(32)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let num_queries = arrayLength(&queries);
    if (idx >= num_queries) {
        return;
    }

    let q = queries[idx];
    let w = i32(params.width);
    let h = i32(params.height);

    // If radius <= 0.0, treat as a point pressure probe (e.g. binaural cockpit wingtip probe)
    if (q.radius <= 0.01) {
        let px = ((i32(round(q.x)) % w) + w) % w;
        let py = ((i32(round(q.y)) % h) + h) % h;
        let s = textureLoad(state_in, vec2<i32>(px, py), 0);
        reductions[idx] = GpuBodyReduction(s.g, s.b, s.a, s.r);
        return;
    }

    let r_cells = min(i32(ceil(q.radius)) + 1, 14);
    let r_sq_max = (q.radius + 1.0) * (q.radius + 1.0);
    let cx = i32(round(q.x));
    let cy = i32(round(q.y));

    var fx_total = 0.0;
    var fy_total = 0.0;
    var tau_total = 0.0;
    var max_p = 0.0;

    for (var oy = -r_cells; oy <= r_cells; oy = oy + 1) {
        for (var ox = -r_cells; ox <= r_cells; ox = ox + 1) {
            let rx = (f32(cx + ox) - q.x);
            let ry = (f32(cy + oy) - q.y);
            if (rx * rx + ry * ry > r_sq_max) {
                continue;
            }

            let gx = ((cx + ox) % w + w) % w;
            let gy = ((cy + oy) % h + h) % h;
            let xe = (gx + 1) % w;
            let xw = (gx + w - 1) % w;
            let yn = (gy + 1) % h;
            let ys = (gy + h - 1) % h;

            let s_c = textureLoad(state_in, vec2<i32>(gx, gy), 0);
            let s_e = textureLoad(state_in, vec2<i32>(xe, gy), 0);
            let s_w = textureLoad(state_in, vec2<i32>(xw, gy), 0);
            let s_n = textureLoad(state_in, vec2<i32>(gx, yn), 0);
            let s_s = textureLoad(state_in, vec2<i32>(gx, ys), 0);

            let p_abs = abs(s_c.r);
            if (p_abs > max_p) {
                max_p = p_abs;
            }

            let grad_px = (s_e.r - s_w.r) * 0.5 * params.inv_dx;
            let grad_py = (s_n.r - s_s.r) * 0.5 * params.inv_dy;
            let grad_mag_sq = grad_px * grad_px + grad_py * grad_py;

            if (grad_mag_sq > 0.04) {
                let q_fx = -(s_e.a - s_w.a) * 0.5 * params.inv_dx;
                let q_fy = -(s_n.a - s_s.a) * 0.5 * params.inv_dy;
                let dfx = -grad_px * 0.18 + q_fx * 0.025;
                let dfy = -grad_py * 0.18 + q_fy * 0.025;

                fx_total += dfx;
                fy_total += dfy;
                tau_total += rx * dfy - ry * dfx;
            } else if (q.radius <= 3.2 && p_abs > 0.015) {
                let q_fx = -(s_e.a - s_w.a) * 0.5 * params.inv_dx;
                let q_fy = -(s_n.a - s_s.a) * 0.5 * params.inv_dy;
                let dfx = q_fx * 0.045;
                let dfy = q_fy * 0.045;

                fx_total += dfx;
                fy_total += dfy;
                tau_total += rx * dfy - ry * dfx;
            }
        }
    }

    reductions[idx] = GpuBodyReduction(fx_total, fy_total, tau_total, max_p);
}
