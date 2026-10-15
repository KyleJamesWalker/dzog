//! Grounded Building Wreckage & Blast Rubble (`crates/shmup/src/render/wreckage.rs`).

use crate::entities::GroundedWreckage;
use super::{check_subpixel_edge, quantize_vga, write_px};

/// Render grounded building wreckage and its scorched blast trail on the terrain/tarmac.
pub fn draw_grounded_wreckage_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    gw: &GroundedWreckage,
) {
    let s_f = scale as f32;
    let steps = 12usize;
    let base_r = ((gw.body.radius * 0.45 * s_f) as isize).max(2);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let gouge_r = ((base_r as f32 * (0.55 + 0.45 * t)) as isize).max(2);
        let gx = ((gw.skid_start_x * (1.0 - t) + gw.body.x * t) * s_f).round() as isize;
        let gy = ((gw.skid_start_y * (1.0 - t) + gw.body.y * t) * s_f).round() as isize;
        for oy in -gouge_r..=gouge_r {
            for ox in -gouge_r..=gouge_r {
                let px = gx + ox;
                let py = gy + oy;
                if px < 12 || px >= rw as isize - 12 || py < 0 || py >= rh as isize {
                    continue;
                }
                let d2 = ox * ox + oy * oy;
                if d2 <= gouge_r * gouge_r {
                    let p_idx = (py as usize * rw + px as usize) * 4;
                    if d2 == gouge_r * gouge_r
                        && (ox + oy + i as isize) % 5 == 0
                        && gw.ember_heat > 0.3
                    {
                        pixel_buffer[p_idx] = 225;
                        pixel_buffer[p_idx + 1] = 95;
                        pixel_buffer[p_idx + 2] = 24;
                    } else {
                        pixel_buffer[p_idx] = (pixel_buffer[p_idx] as f32 * 0.34) as u8;
                        pixel_buffer[p_idx + 1] = (pixel_buffer[p_idx + 1] as f32 * 0.30) as u8;
                        pixel_buffer[p_idx + 2] = (pixel_buffer[p_idx + 2] as f32 * 0.28) as u8;
                    }
                }
            }
        }
    }

    let body = &gw.body;
    let persp = 0.74f32;
    let step = 1.0 / (s_f * persp);
    let r_sub = ((body.radius * persp + 1.2) * s_f).ceil() as isize;
    let cx_sub = (body.x * s_f).round() as isize;
    let cy_sub = (body.y * s_f).round() as isize;
    let cos_t = body.theta.cos();
    let sin_t = body.theta.sin();
    let center = (body.local_size as f32 - 1.0) * 0.5;

    for oy in -r_sub..=r_sub {
        for ox in -r_sub..=r_sub {
            let px = cx_sub + ox;
            let py = cy_sub + oy;
            if px < 12 || px >= rw as isize - 12 || py < 0 || py >= rh as isize {
                continue;
            }
            let dx = ox as f32 * step;
            let dy = oy as f32 * step;
            let lx = dx * cos_t + dy * sin_t + center;
            let ly = -dx * sin_t + dy * cos_t + center;
            let coh = body.sample_local_cohesion(lx, ly);
            if coh < 0.15 {
                continue;
            }

            let (is_edge, _is_fracture_edge) = check_subpixel_edge(body, lx, ly, step * 1.15);
            if is_edge {
                write_px(pixel_buffer, rw, px, py, [16, 18, 22]);
                continue;
            }

            let ember = if coh < 0.58 {
                gw.ember_heat * (0.58 - coh) * 1.8
            } else {
                0.0
            };
            let r = (0.28 + ember * 0.65).clamp(0.0, 1.0);
            let g = (0.26 + ember * 0.25).clamp(0.0, 1.0);
            let b = 0.29f32;
            write_px(
                pixel_buffer,
                rw,
                px,
                py,
                [quantize_vga(r), quantize_vga(g), quantize_vga(b)],
            );
        }
    }
}
