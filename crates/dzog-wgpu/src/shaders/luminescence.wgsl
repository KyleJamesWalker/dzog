// Screened reaction-diffusion luminescence field compute shader.
// Pass 3: Evolves exciton luminescence L(x, y) driven by wave energy (p, v),
// material wave speed c = sqrt(K / rho), and piezoelectric crystal stress.

struct SimParams {
    width: u32,
    height: u32,
    dx: f32,
    dy: f32,
    dt: f32,
    inv_dx: f32,
    inv_dy: f32,
    outer_rule: u32, // 0 = RigidWall, 1 = AbsorbingSponge, 2 = Periodic
    beta: f32,
    alpha: f32,
    threshold_decay: f32,
    _pad: f32,
}

@group(0) @binding(0) var<uniform> params: SimParams;
@group(0) @binding(1) var state_in: texture_2d<f32>;    // RGBA: (p, vx, vy, Q)
@group(0) @binding(2) var material: texture_2d<f32>;    // RGBA: (rho, K, gamma, cohesion)
@group(0) @binding(3) var lum_in: texture_2d<f32>;      // RGBA: (L, 0, 0, 0)
@group(0) @binding(4) var lum_out: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = global_id.x;
    let y = global_id.y;

    if (x >= params.width || y >= params.height) {
        return;
    }

    let w = params.width;
    let h = params.height;
    let coord = vec2<i32>(i32(x), i32(y));

    let cur_state = textureLoad(state_in, coord, 0);
    let cur_mat = textureLoad(material, coord, 0);
    let l_c = textureLoad(lum_in, coord, 0).r;

    var l_e = 0.0;
    var l_w = 0.0;
    var l_n = 0.0;
    var l_s = 0.0;

    if (params.outer_rule == 2u) {
        let xe = i32((x + 1u) % w);
        let xw = i32((x + w - 1u) % w);
        let yn = i32((y + 1u) % h);
        let ys = i32((y + h - 1u) % h);
        l_e = textureLoad(lum_in, vec2<i32>(xe, i32(y)), 0).r;
        l_w = textureLoad(lum_in, vec2<i32>(xw, i32(y)), 0).r;
        l_n = textureLoad(lum_in, vec2<i32>(i32(x), yn), 0).r;
        l_s = textureLoad(lum_in, vec2<i32>(i32(x), ys), 0).r;
    } else {
        if (x + 1u < w) {
            l_e = textureLoad(lum_in, vec2<i32>(i32(x + 1u), i32(y)), 0).r;
        }
        if (x > 0u) {
            l_w = textureLoad(lum_in, vec2<i32>(i32(x - 1u), i32(y)), 0).r;
        }
        if (y + 1u < h) {
            l_n = textureLoad(lum_in, vec2<i32>(i32(x), i32(y + 1u)), 0).r;
        }
        if (y > 0u) {
            l_s = textureLoad(lum_in, vec2<i32>(i32(x), i32(y - 1u)), 0).r;
        }
    }

    let inv_dx2 = params.inv_dx * params.inv_dx;
    let inv_dy2 = params.inv_dy * params.inv_dy;
    let lap_l = (l_e - 2.0 * l_c + l_w) * inv_dx2 + (l_n - 2.0 * l_c + l_s) * inv_dy2;

    let rho = cur_mat.r;
    let k = cur_mat.g;
    let coh = cur_mat.a;
    let c_speed = sqrt(k / rho);

    var diff_coeff = 0.18 * c_speed;
    if (coh >= 0.15 && coh < 0.55) {
        diff_coeff = 0.26 * c_speed;
    }

    let p_abs = abs(cur_state.r);
    let vx = cur_state.g;
    let vy = cur_state.b;
    let v_sq = vx * vx + vy * vy;

    var wave_src = 0.0;
    if (p_abs > 0.24) {
        wave_src = (p_abs - 0.24) * 0.55 + v_sq * 0.28;
    }

    var piezo_src = 0.0;
    if (coh >= 0.15 && p_abs > 0.12) {
        var piezo_gain = 0.75;
        if (rho < 0.90) {
            piezo_gain = 1.35;
        }
        piezo_src = p_abs * (1.25 - 0.45 * coh) * piezo_gain;
    }

    let decay_l = exp(-1.35 * params.dt);
    let l_next = clamp(
        decay_l * (l_c + diff_coeff * lap_l * params.dt) + (wave_src + piezo_src) * params.dt,
        0.0,
        4.0
    );

    textureStore(lum_out, coord, vec4<f32>(l_next, 0.0, 0.0, 0.0));
}
