// GPU-native Gaussian / Directional Wave Packet excitation compute shader.
// Reads State A and Material, injects pressure + impedance-matched directional velocity into State B.

struct ImpulseParams {
    width: u32,
    height: u32,
    center_x: f32,
    center_y: f32,
    two_sigma_sq: f32,
    amplitude: f32,
    dir_x: f32,
    dir_y: f32,
}

@group(0) @binding(0) var<uniform> impulse: ImpulseParams;
@group(0) @binding(1) var state_in: texture_2d<f32>;    // RGBA: (p, vx, vy, Q)
@group(0) @binding(2) var material: texture_2d<f32>;    // RGBA: (rho, K, gamma, cohesion)
@group(0) @binding(3) var state_out: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = global_id.x;
    let y = global_id.y;

    if (x >= impulse.width || y >= impulse.height) {
        return;
    }

    let coord = vec2<i32>(i32(x), i32(y));
    let cur_state = textureLoad(state_in, coord, 0);
    let cur_mat = textureLoad(material, coord, 0);

    let dx = f32(x) - impulse.center_x;
    let dy = f32(y) - impulse.center_y;
    let dist_sq = dx * dx + dy * dy;

    var p_new = cur_state.r;
    var vx_new = cur_state.g;
    var vy_new = cur_state.b;

    // 4-sigma cutoff radius
    if (dist_sq <= 8.0 * impulse.two_sigma_sq) {
        let p_val = impulse.amplitude * exp(-dist_sq / impulse.two_sigma_sq);
        p_new += p_val;

        if (abs(impulse.dir_x) > 0.0001 || abs(impulse.dir_y) > 0.0001) {
            let z = sqrt(max(cur_mat.r * cur_mat.g, 0.0001));
            vx_new += (p_val / z) * impulse.dir_x;
            vy_new += (p_val / z) * impulse.dir_y;
        }
    }

    textureStore(state_out, coord, vec4<f32>(p_new, vx_new, vy_new, cur_state.a));
}
