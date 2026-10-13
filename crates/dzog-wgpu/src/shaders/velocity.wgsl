// Yee-staggered acoustic velocity compute shader.
// Pass 1: Reads State A (pressure, old velocity), writes State B (pressure preserved, updated velocity).

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
@group(0) @binding(1) var state_in: texture_2d<f32>;    // RGBA: (p, vx, vy, _)
@group(0) @binding(2) var material: texture_2d<f32>;    // RGBA: (rho, K, gamma, _)
@group(0) @binding(3) var boundary: texture_2d<f32>;    // RGBA: (wall_east, wall_vx, wall_north, wall_vy)
@group(0) @binding(4) var state_out: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = global_id.x;
    let y = global_id.y;

    if (x >= params.width || y >= params.height) {
        return;
    }

    let coord = vec2<i32>(i32(x), i32(y));
    let cur_state = textureLoad(state_in, coord, 0);
    let cur_mat = textureLoad(material, coord, 0);
    let cur_bound = textureLoad(boundary, coord, 0);

    let p_center = cur_state.r;
    var vx_new = cur_state.g;
    var vy_new = cur_state.b;

    // --- East-face velocity update (vx) ---
    let wall_e_blocked = cur_bound.r > 0.5;
    let wall_e_vx = cur_bound.g;

    if (wall_e_blocked) {
        vx_new = wall_e_vx;
    } else if (x + 1u < params.width) {
        let coord_r = vec2<i32>(i32(x + 1u), i32(y));
        let p_right = textureLoad(state_in, coord_r, 0).r;
        let mat_r = textureLoad(material, coord_r, 0);

        let grad_p_x = (p_right - p_center) * params.inv_dx;
        let rho_face = 0.5 * (cur_mat.r + mat_r.r);
        let gamma_face = 0.5 * (cur_mat.b + mat_r.b);

        let decay = exp(-gamma_face * params.dt);
        let accel = -grad_p_x / rho_face;
        vx_new = decay * cur_state.g + accel * params.dt;
    } else {
        // Outer east border
        if (params.outer_rule == 2u) {
            let coord_wrap = vec2<i32>(0, i32(y));
            let p_wrap = textureLoad(state_in, coord_wrap, 0).r;
            let mat_wrap = textureLoad(material, coord_wrap, 0);
            let grad_p_x = (p_wrap - p_center) * params.inv_dx;
            let rho_face = 0.5 * (cur_mat.r + mat_wrap.r);
            let gamma_face = 0.5 * (cur_mat.b + mat_wrap.b);
            let decay = exp(-gamma_face * params.dt);
            let accel = -grad_p_x / rho_face;
            vx_new = decay * cur_state.g + accel * params.dt;
        } else {
            vx_new = 0.0;
        }
    }

    // --- North-face velocity update (vy) ---
    let wall_n_blocked = cur_bound.b > 0.5;
    let wall_n_vy = cur_bound.a;

    if (wall_n_blocked) {
        vy_new = wall_n_vy;
    } else if (y + 1u < params.height) {
        let coord_t = vec2<i32>(i32(x), i32(y + 1u));
        let p_top = textureLoad(state_in, coord_t, 0).r;
        let mat_t = textureLoad(material, coord_t, 0);

        let grad_p_y = (p_top - p_center) * params.inv_dy;
        let rho_face = 0.5 * (cur_mat.r + mat_t.r);
        let gamma_face = 0.5 * (cur_mat.b + mat_t.b);

        let decay = exp(-gamma_face * params.dt);
        let accel = -grad_p_y / rho_face;
        vy_new = decay * cur_state.b + accel * params.dt;
    } else {
        // Outer north border
        if (params.outer_rule == 2u) {
            let coord_wrap = vec2<i32>(i32(x), 0);
            let p_wrap = textureLoad(state_in, coord_wrap, 0).r;
            let mat_wrap = textureLoad(material, coord_wrap, 0);
            let grad_p_y = (p_wrap - p_center) * params.inv_dy;
            let rho_face = 0.5 * (cur_mat.r + mat_wrap.r);
            let gamma_face = 0.5 * (cur_mat.b + mat_wrap.b);
            let decay = exp(-gamma_face * params.dt);
            let accel = -grad_p_y / rho_face;
            vy_new = decay * cur_state.b + accel * params.dt;
        } else {
            vy_new = 0.0;
        }
    }

    // --- Bohm-Hiley Quantum Potential Q = -Laplacian(R) / R ---
    var q_bohm = 0.0;
    if (params.outer_rule == 2u || (x > 0u && x + 1u < params.width && y > 0u && y + 1u < params.height)) {
        let eps_sq = 0.0064; // (0.08)^2
        let xe = i32((x + 1u) % params.width);
        let xw = i32((x + params.width - 1u) % params.width);
        let yn = i32((y + 1u) % params.height);
        let ys = i32((y + params.height - 1u) % params.height);

        let p_e = textureLoad(state_in, vec2<i32>(xe, i32(y)), 0).r;
        let p_w = textureLoad(state_in, vec2<i32>(xw, i32(y)), 0).r;
        let p_n = textureLoad(state_in, vec2<i32>(i32(x), yn), 0).r;
        let p_s = textureLoad(state_in, vec2<i32>(i32(x), ys), 0).r;

        let r_c = sqrt(p_center * p_center + eps_sq);
        let r_e = sqrt(p_e * p_e + eps_sq);
        let r_w = sqrt(p_w * p_w + eps_sq);
        let r_n = sqrt(p_n * p_n + eps_sq);
        let r_s = sqrt(p_s * p_s + eps_sq);

        let inv_dx2 = params.inv_dx * params.inv_dx;
        let inv_dy2 = params.inv_dy * params.inv_dy;
        let lap_r = (r_e - 2.0 * r_c + r_w) * inv_dx2 + (r_n - 2.0 * r_c + r_s) * inv_dy2;
        q_bohm = -lap_r / r_c;
    }

    // Write updated state: pressure preserved, new velocities, Bohmian potential Q in .a
    textureStore(state_out, coord, vec4<f32>(p_center, vx_new, vy_new, q_bohm));
}
