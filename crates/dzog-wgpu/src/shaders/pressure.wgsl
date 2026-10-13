// Yee-staggered acoustic pressure compute shader.
// Pass 2: Reads State B (new velocity, old pressure), writes State A (updated pressure, preserved new velocity).

struct SimParams {
    width: u32,
    height: u32,
    dx: f32,
    dy: f32,
    dt: f32,
    inv_dx: f32,
    inv_dy: f32,
    outer_rule: u32, // 0 = RigidWall, 1 = AbsorbingSponge, 2 = Periodic
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

    let vx_e = cur_state.g;
    let vy_n = cur_state.b;

    // West face velocity vx_w
    var vx_w = 0.0;
    if (x > 0u) {
        let coord_l = vec2<i32>(i32(x - 1u), i32(y));
        vx_w = textureLoad(state_in, coord_l, 0).g;
    } else {
        if (params.outer_rule == 2u) {
            let coord_wrap = vec2<i32>(i32(params.width - 1u), i32(y));
            vx_w = textureLoad(state_in, coord_wrap, 0).g;
        }
    }

    // South face velocity vy_s
    var vy_s = 0.0;
    if (y > 0u) {
        let coord_b = vec2<i32>(i32(x), i32(y - 1u));
        vy_s = textureLoad(state_in, coord_b, 0).b;
    } else {
        if (params.outer_rule == 2u) {
            let coord_wrap = vec2<i32>(i32(x), i32(params.height - 1u));
            vy_s = textureLoad(state_in, coord_wrap, 0).b;
        }
    }

    // Divergence of particle velocity: div(v) = dvx/dx + dvy/dy
    let div_v = (vx_e - vx_w) * params.inv_dx + (vy_n - vy_s) * params.inv_dy;

    let k = cur_mat.g;
    let gamma = cur_mat.b;
    let decay = exp(-gamma * params.dt);

    let p_new = decay * cur_state.r - (k * params.dt) * div_v;

    // Write updated state: new pressure, preserved velocities
    textureStore(state_out, coord, vec4<f32>(p_new, vx_e, vy_n, 0.0));
}
