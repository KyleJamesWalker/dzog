//! Sky Actor Drop-Shadows (`crates/shmup/src/render/shadows.rs`).
//!
//! Ground building and terrain cliff shadows are computed automatically in `O(cells)` via
//! `dzog_core::MaterialField::recompute_horizon_shadows`. This helper projects offset
//! silhouettes for airborne actors (`z > h_surface`).

use dzog_core::CohesiveBody;
use crate::field_game::ShmupFieldGame;

impl ShmupFieldGame {
    /// Project a smooth sub-cell 2.5D silhouette shadow onto the ground/water layer.
    pub(crate) fn draw_body_shadow_subcell(
        pixel_buffer: &mut [u8],
        rw: usize,
        rh: usize,
        scale: usize,
        body: &CohesiveBody,
        offset_x_cells: f32,
        offset_y_cells: f32,
        darken: f32,
    ) {
        let s_f = scale as f32;
        let inv_s = 1.0 / s_f;
        let r_sub = ((body.radius + 1.2) * s_f).ceil() as isize;
        let cx_sub = ((body.x + offset_x_cells) * s_f).round() as isize;
        let cy_sub = ((body.y + offset_y_cells) * s_f).round() as isize;
        let cos_t = body.theta.cos();
        let sin_t = body.theta.sin();
        let center = (body.local_size as f32 - 1.0) * 0.5;

        for oy in -r_sub..=r_sub {
            for ox in -r_sub..=r_sub {
                let sx = cx_sub + ox;
                let sy = cy_sub + oy;
                if sx < 12 || sx >= rw as isize - 12 || sy < 0 || sy >= rh as isize {
                    continue;
                }
                let dx = ox as f32 * inv_s;
                let dy = oy as f32 * inv_s;
                let lx = dx * cos_t + dy * sin_t + center;
                let ly = -dx * sin_t + dy * cos_t + center;
                if body.sample_local_cohesion(lx, ly) < 0.15 {
                    continue;
                }
                let p_idx = (sy as usize * rw + sx as usize) * 4;
                pixel_buffer[p_idx] = (pixel_buffer[p_idx] as f32 * darken) as u8;
                pixel_buffer[p_idx + 1] = (pixel_buffer[p_idx + 1] as f32 * darken) as u8;
                pixel_buffer[p_idx + 2] = (pixel_buffer[p_idx + 2] as f32 * darken) as u8;
            }
        }
    }
}
