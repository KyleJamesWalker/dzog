// Fullscreen visualization render shader for Dzoglang acoustic field.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Generate fullscreen triangle: (0,0), (2,0), (0,2) in NDC coordinates (-1..1)
    let x = f32((in_vertex_index << 1u) & 2u);
    let y = f32(in_vertex_index & 2u);
    out.clip_position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@group(0) @binding(0) var state_tex: texture_2d<f32>;
@group(0) @binding(1) var bound_tex: texture_2d<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let dims = textureDimensions(state_tex);
    let coord = vec2<i32>(
        clamp(i32(in.uv.x * f32(dims.x)), 0, i32(dims.x - 1u)),
        clamp(i32(in.uv.y * f32(dims.y)), 0, i32(dims.y - 1u))
    );

    let state = textureLoad(state_tex, coord, 0);
    let bound = textureLoad(bound_tex, coord, 0);

    let p = state.r;
    let vx = state.g;
    let vy = state.b;
    let v_mag = sqrt(vx * vx + vy * vy);

    // Wall highlighting
    let is_wall = bound.r > 0.5 || bound.b > 0.5;
    if (is_wall) {
        return vec4<f32>(0.95, 0.95, 0.98, 1.0);
    }

    // Diverging acoustic colormap:
    // Positive pressure: vibrant gold / crimson
    // Negative pressure: deep neon cyan / cobalt blue
    // Velocity magnitude adds a soft ambient emerald/violet energy glow
    var col = vec3<f32>(0.02, 0.03, 0.06); // Deep space background

    if (p > 0.0) {
        let t = clamp(p * 2.0, 0.0, 1.0);
        col = mix(col, vec3<f32>(1.0, 0.45, 0.15), t); // Warm amber
        if (p > 0.5) {
            let t2 = clamp((p - 0.5) * 2.0, 0.0, 1.0);
            col = mix(col, vec3<f32>(1.0, 0.95, 0.8), t2); // High intensity white-hot
        }
    } else {
        let t = clamp(-p * 2.0, 0.0, 1.0);
        col = mix(col, vec3<f32>(0.1, 0.65, 1.0), t); // Cool electric cyan
        if (-p > 0.5) {
            let t2 = clamp((-p - 0.5) * 2.0, 0.0, 1.0);
            col = mix(col, vec3<f32>(0.7, 0.9, 1.0), t2); // High intensity frozen white
        }
    }

    // Subtle particle kinetic energy glow
    col += vec3<f32>(0.0, 0.3 * clamp(v_mag * 1.5, 0.0, 1.0), 0.15 * clamp(v_mag * 1.5, 0.0, 1.0));

    return vec4<f32>(col, 1.0);
}
