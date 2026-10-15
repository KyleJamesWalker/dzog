//! 2.5D Sub-Cell (`4x` = `960x540`) Field, Terrain & VGA Sprite Renderer (`crates/shmup/src/render/mod.rs`).
//!
//! Organized into focused submodules:
//! - [`hud`]: Vertical sidebars, `$00053450` bounty ticker, and ordnance badge.
//! - [`shadows`]: 2.5D sky drop-shadows.
//! - [`ships`]: Player interceptor and enemy squadron VGA rendering.
//! - [`structures`]: Ground domes, biodomes, turrets, patrol boats, canisters, and projectiles.
//! - [`wreckage`]: Grounded building rubble and blast trails.

pub mod hud;
pub mod shadows;
pub mod ships;
pub mod structures;
pub mod wreckage;

use dzog_bevy::FieldViewMode;
use dzog_core::{CohesiveBody, MineralClass};

use crate::canyon::{canyon_wall_bounds, sample_ground_cell, GroundCellKind};
use crate::field_game::ShmupFieldGame;
use self::ships::{draw_alien_vga, draw_player_raptor_vga};
use self::structures::{
    draw_asteroid_or_shard_vga, draw_bolts_vga, draw_ground_target_vga, draw_power_cube_vga,
};
use self::wreckage::draw_grounded_wreckage_vga;

#[inline]
pub(crate) fn quantize_vga(v: f32) -> u8 {
    let b = (v.clamp(0.0, 1.0) * 255.0) as u8;
    (b >> 3) << 3
}

#[inline]
pub(crate) fn write_px(buf: &mut [u8], rw: usize, px: isize, py: isize, rgb: [u8; 3]) {
    let idx = (py as usize * rw + px as usize) * 4;
    buf[idx] = rgb[0];
    buf[idx + 1] = rgb[1];
    buf[idx + 2] = rgb[2];
    buf[idx + 3] = 255;
}

#[inline]
pub(crate) fn check_subpixel_edge(
    body: &CohesiveBody,
    lx: f32,
    ly: f32,
    step: f32,
) -> (bool, bool) {
    // Fast-path interior texels: since bilinear cohesion changes by at most `step` (~0.28)
    // over `step` distance, any texel with `coh >= 0.48` cannot border `coh < 0.15`.
    if body.sample_local_cohesion(lx, ly) >= 0.48 {
        return (false, false);
    }
    let neighbors = [
        (lx - step, ly),
        (lx + step, ly),
        (lx, ly - step),
        (lx, ly + step),
    ];
    let n = body.local_size as isize;
    for (nlx, nly) in neighbors {
        if body.sample_local_cohesion(nlx, nly) < 0.15 {
            let ix = nlx.round() as isize;
            let iy = nly.round() as isize;
            let was_occupied = if ix >= 0 && ix < n && iy >= 0 && iy < n {
                body.occupied[(iy as usize) * body.local_size + (ix as usize)]
            } else {
                false
            };
            return (true, was_occupied);
        }
    }
    (false, false)
}

impl ShmupFieldGame {
    /// Render the dual-layer ground/sky ecology into `self.pixel_buffer`.
    ///
    /// Performance optimization: evaluates `canyon_wall_bounds` once per lattice row (`135` rows)
    /// and `sample_ground_cell` once per lattice cell (`240x135 = 32,400` cells) rather than per
    /// sub-pixel (`960x540 = 518,400`), reducing background render time by ~14x.
    pub fn render_to_pixel_buffer(&mut self) {
        let scale = Self::RENDER_SCALE;
        let s_f = scale as f32;
        let inv_s = 1.0 / s_f;
        let w = self.width;
        let h = self.height;
        let rw = self.render_width();
        let rh = self.render_height();

        if self.pixel_buffer.len() != rw * rh * 4 {
            self.pixel_buffer.resize(rw * rh * 4, 0);
        }

        let pipe_flash = (self.pipeline_flash_timer as f32) / 16.0;
        let scroll = self.canyon_scroll;
        let has_lum = self.solver.state.luminescence.len() == w * h;
        let has_shadows = self.solver.material.shadow_horizon.len() == w * h
            && self.solver.material.elevation.len() == w * h;

        if self.view_mode != FieldViewMode::CompositeArcade {
            for gy in 0..h {
                for gx in 0..w {
                    let idx = gy * w + gx;
                    let p = self.solver.state.pressure[idx];
                    let vx = self.solver.state.vx[idx];
                    let vy = self.solver.state.vy[idx];
                    let v_mag = (vx * vx + vy * vy).sqrt();
                    let lum = if has_lum {
                        self.solver.state.luminescence[idx]
                    } else {
                        0.0
                    };
                    let rho = self.solver.material.density[idx];
                    let k = self.solver.material.bulk_modulus[idx];
                    let coh = self.solver.material.cohesion[idx];

                    let (r, g, b) = match self.view_mode {
                        FieldViewMode::PressureAndBohmian => {
                            let q = self.solver.state.quantum_potential_at(gx, gy);
                            let p_pos = p.max(0.0);
                            let p_neg = (-p).max(0.0);
                            let q_well = (-q).max(0.0) * 0.09;
                            (
                                0.03 + p_pos * 0.65 + q_well * 0.55 + lum * 0.20,
                                0.03 + q_well * 0.88 + lum * 0.45,
                                0.06 + p_neg * 0.70 + q_well * 0.42 + lum * 0.55,
                            )
                        }
                        FieldViewMode::VelocityAndFlux => {
                            let flux = p.abs() * v_mag;
                            (
                                0.03 + flux * 0.68 + lum * 0.30,
                                0.04 + v_mag * 0.78 + lum * 0.50,
                                0.07 + v_mag * 0.48 + lum * 0.42,
                            )
                        }
                        FieldViewMode::ImpedanceAndCohesion => {
                            let z = (rho * k).sqrt();
                            let z_dev = (z - 1.0).abs();
                            (
                                0.04 + z_dev * 0.70,
                                0.04 + coh * 0.68,
                                0.07 + (1.0 - coh) * z_dev * 0.85,
                            )
                        }
                        FieldViewMode::CompositeArcade => unreachable!(),
                    };

                    let px_col = [
                        (r.clamp(0.0, 1.0) * 255.0) as u8,
                        (g.clamp(0.0, 1.0) * 255.0) as u8,
                        (b.clamp(0.0, 1.0) * 255.0) as u8,
                        255,
                    ];
                    let base_py = gy * scale;
                    let base_px = gx * scale;
                    for sy in 0..scale {
                        let row_off = (base_py + sy) * rw + base_px;
                        for sx in 0..scale {
                            let p_idx = (row_off + sx) * 4;
                            self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&px_col);
                        }
                    }
                }
            }
        } else {
            // Fast cell-hoisted CompositeArcade ground pass (`32,400` cell samples -> `518,400` sub-pixels)
            for gy in 0..h {
                let (left_edge, right_edge, canyon_class) = canyon_wall_bounds(gy, h, w, scroll);
                let base_py = gy * scale;

                for gx in 0..w {
                    let g_cell = sample_ground_cell(gx, gy, w, h, scroll);
                    let base_px = gx * scale;
                    let idx = gy * w + gx;
                    let cell_s_depth = if has_shadows && gx >= left_edge && gx < right_edge {
                        (self.solver.material.shadow_horizon[idx]
                            - self.solver.material.elevation[idx])
                            .max(0.0)
                    } else {
                        0.0
                    };
                    let check_fine_shadow = cell_s_depth > 0.001
                        || (has_shadows
                            && gx + 1 < right_edge
                            && (self.solver.material.shadow_horizon[idx + 1]
                                > self.solver.material.elevation[idx + 1]));

                    // Fast path for cells that don't have sub-pixel platform seams or shadow gradients (~82% of cells)
                    let is_uniform_cell = !check_fine_shadow
                        && !matches!(
                            g_cell.kind,
                            GroundCellKind::IndustrialPlatform | GroundCellKind::WaveguidePipeline
                        );

                    if is_uniform_cell {
                        let wx = gx as f32 + 0.5;
                        let wy_i = ((gy as f32 - scroll) * 2.0) as i32 as u32;
                        let wx_i = (wx * 2.0) as u32;
                        let mut h_val = wx_i
                            .wrapping_mul(374761393)
                            .wrapping_add(wy_i.wrapping_mul(668265263));
                        h_val = (h_val ^ (h_val >> 13)).wrapping_mul(1274126177);
                        let grain = (((h_val >> 16) & 7) as f32 - 3.5) * 0.003;

                        let (r, g, b) = match g_cell.kind {
                            GroundCellKind::CanyonWall(_) => {
                                let rim_dist = if gx < left_edge {
                                    left_edge as f32 - wx
                                } else {
                                    wx - right_edge as f32 + 1.0
                                };
                                let rim_glow =
                                    (1.0 / (0.6 + rim_dist.max(0.0) * 0.45)).clamp(0.0, 1.0);
                                let base = match canyon_class {
                                    MineralClass::ObsidianAnvil => (0.10, 0.12, 0.19),
                                    MineralClass::BismuthCrystal => (0.15, 0.12, 0.23),
                                    _ => (0.09, 0.19, 0.25),
                                };
                                (
                                    base.0 + rim_glow * 0.16,
                                    base.1 + rim_glow * 0.30,
                                    base.2 + rim_glow * 0.44,
                                )
                            }
                            GroundCellKind::HydroCanal => {
                                let trim = if g_cell.is_border_trim { 0.13 } else { 0.0 };
                                let bank_shade = if g_cell.sub_feature == 1 { 0.68 } else { 1.0 };
                                (
                                    (0.04 + grain + trim * 0.8) * bank_shade,
                                    (0.19 + grain * 1.4 + trim * 0.48) * bank_shade,
                                    (0.29 + grain * 1.8) * bank_shade,
                                )
                            }
                            GroundCellKind::TerraBasalt => {
                                if g_cell.sub_feature == 1 {
                                    (0.07 + grain, 0.22 + grain * 2.0, 0.08 + grain)
                                } else {
                                    (0.14 + grain, 0.095 + grain, 0.105 + grain)
                                }
                            }
                            _ => unreachable!(),
                        };

                        let rgba = [
                            (r.clamp(0.0, 1.0) * 255.0) as u8,
                            (g.clamp(0.0, 1.0) * 255.0) as u8,
                            (b.clamp(0.0, 1.0) * 255.0) as u8,
                            255,
                        ];
                        for sy in 0..scale {
                            let row_off = (base_py + sy) * rw + base_px;
                            for sx in 0..scale {
                                let p_idx = (row_off + sx) * 4;
                                self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&rgba);
                            }
                        }
                        continue;
                    }

                    for sy in 0..scale {
                        let py = base_py + sy;
                        let wy = (py as f32 + 0.5) * inv_s;
                        let wy_scrolled = wy - scroll;
                        let wy_i = (wy_scrolled * 2.0).floor() as u32;
                        let row_off = py * rw + base_px;

                        for sx in 0..scale {
                            let px = base_px + sx;
                            let wx = (px as f32 + 0.5) * inv_s;
                            let wx_i = (wx * 2.0).floor() as u32;
                            let mut h_val = wx_i
                                .wrapping_mul(374761393)
                                .wrapping_add(wy_i.wrapping_mul(668265263));
                            h_val = (h_val ^ (h_val >> 13)).wrapping_mul(1274126177);
                            let grain = (((h_val >> 16) & 7) as f32 - 3.5) * 0.003;

                            let (mut r, mut g, mut b) = match g_cell.kind {
                                GroundCellKind::CanyonWall(_) => {
                                    let rim_dist = if gx < left_edge {
                                        left_edge as f32 - wx
                                    } else {
                                        wx - right_edge as f32 + 1.0
                                    };
                                    let rim_glow =
                                        (1.0 / (0.6 + rim_dist.max(0.0) * 0.45)).clamp(0.0, 1.0);
                                    let base = match canyon_class {
                                        MineralClass::ObsidianAnvil => (0.10, 0.12, 0.19),
                                        MineralClass::BismuthCrystal => (0.15, 0.12, 0.23),
                                        _ => (0.09, 0.19, 0.25),
                                    };
                                    (
                                        base.0 + rim_glow * 0.16,
                                        base.1 + rim_glow * 0.30,
                                        base.2 + rim_glow * 0.44,
                                    )
                                }
                                GroundCellKind::HydroCanal => {
                                    let trim = if g_cell.is_border_trim { 0.13 } else { 0.0 };
                                    let bank_shade = if g_cell.sub_feature == 1 { 0.68 } else { 1.0 };
                                    (
                                        (0.04 + grain + trim * 0.8) * bank_shade,
                                        (0.19 + grain * 1.4 + trim * 0.48) * bank_shade,
                                        (0.29 + grain * 1.8) * bank_shade,
                                    )
                                }
                                GroundCellKind::WaveguidePipeline => {
                                    let sub_rib =
                                        ((wy_scrolled * 3.0).floor() as i32).rem_euclid(3) == 0
                                            || ((wx * 3.0).floor() as i32).rem_euclid(3) == 0;
                                    let rib = if sub_rib { 0.032 } else { -0.025 };
                                    let surge = (pipe_flash * 0.45).clamp(0.0, 1.0);
                                    (
                                        0.14 + rib + surge * 0.45,
                                        0.15 + rib + surge * 0.28,
                                        0.18 + rib + surge * 0.15,
                                    )
                                }
                                GroundCellKind::IndustrialPlatform => {
                                    let sub_seam =
                                        ((wy_scrolled * 3.0).floor() as i32).rem_euclid(24) == 0
                                            || (px as i32).rem_euclid(28) == 0;
                                    let seam = if sub_seam { -0.055 } else { 0.0 };
                                    let bay = if g_cell.sub_feature == 1 { -0.045 } else { 0.0 };
                                    let trim = if g_cell.is_border_trim { 0.16 } else { 0.0 };
                                    (
                                        0.19 + seam + bay + grain + trim * 0.95,
                                        0.18 + seam + bay + grain + trim * 0.45,
                                        0.21 + seam + bay + grain,
                                    )
                                }
                                GroundCellKind::TerraBasalt => {
                                    if g_cell.sub_feature == 1 {
                                        (0.07 + grain, 0.22 + grain * 2.0, 0.08 + grain)
                                    } else {
                                        (0.14 + grain, 0.095 + grain, 0.105 + grain)
                                    }
                                }
                            };

                            if check_fine_shadow {
                                let s_depth = self.solver.material.shadow_depth_at(wx, wy);
                                if s_depth > 0.02 {
                                    let shade =
                                        (1.0 - (s_depth * 0.85).clamp(0.0, 0.52)).clamp(0.48, 1.0);
                                    r *= shade;
                                    g *= shade;
                                    b *= shade;
                                }
                            }

                            let p_idx = (row_off + sx) * 4;
                            self.pixel_buffer[p_idx] = (r.clamp(0.0, 1.0) * 255.0) as u8;
                            self.pixel_buffer[p_idx + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
                            self.pixel_buffer[p_idx + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
                            self.pixel_buffer[p_idx + 3] = 255;
                        }
                    }
                }
            }
        }

        // 2. Persistent Scorched Craters (trig-free stepped VGA soot pit)
        for crater in &self.craters {
            let cx = (crater.x * s_f).round() as isize;
            let cy = (crater.y * s_f).round() as isize;
            let r_sub = ((crater.radius * s_f).ceil() as isize) + 1;
            let inv_r2 = 1.0 / (crater.radius * s_f).max(1.0).powi(2);
            for oy in -r_sub..=r_sub {
                for ox in -r_sub..=r_sub {
                    let px = cx + ox;
                    let py = cy + oy;
                    if px < 12 || px >= rw as isize - 12 || py < 0 || py >= rh as isize {
                        continue;
                    }
                    let d2 = (ox * ox + oy * oy) as f32;
                    let jag = 0.85 + (((ox * 7 ^ oy * 13) & 3) as f32) * 0.08;
                    let norm2 = d2 * inv_r2 * jag;
                    if norm2 <= 1.0 {
                        let p_idx = (py as usize * rw + px as usize) * 4;
                        let step_char = if norm2 < 0.18 {
                            0.20
                        } else if norm2 < 0.60 {
                            0.36
                        } else {
                            0.58
                        };
                        let is_ember_speck = norm2 < 0.30
                            && crater.ember_heat > 0.35
                            && ((px * 7 + py * 13) % 29 == 0);
                        if is_ember_speck {
                            self.pixel_buffer[p_idx] = 216;
                            self.pixel_buffer[p_idx + 1] = 88;
                            self.pixel_buffer[p_idx + 2] = 24;
                        } else {
                            self.pixel_buffer[p_idx] =
                                (self.pixel_buffer[p_idx] as f32 * step_char) as u8;
                            self.pixel_buffer[p_idx + 1] =
                                (self.pixel_buffer[p_idx + 1] as f32 * step_char) as u8;
                            self.pixel_buffer[p_idx + 2] =
                                (self.pixel_buffer[p_idx + 2] as f32 * step_char) as u8;
                        }
                    }
                }
            }
        }

        // 2b. Grounded Building Wreckage
        for gw in &self.grounded_wreckage {
            draw_grounded_wreckage_vga(&mut self.pixel_buffer, rw, rh, scale, gw);
        }

        // 3. Ground Layer Structures
        let player_pos = (self.player_hull.x, self.player_hull.y);
        for gt in &self.ground_targets {
            draw_ground_target_vga(&mut self.pixel_buffer, rw, rh, scale, gt, player_pos);
        }

        // 4. 2.5D Oblique Sky Drop-Shadows (`dx = -5.0, dy = +6.0` cells)
        for rock in &self.asteroids {
            Self::draw_body_shadow_subcell(
                &mut self.pixel_buffer,
                rw,
                rh,
                scale,
                rock,
                -4.2,
                5.2,
                0.42,
            );
        }
        for alien in &self.aliens {
            Self::draw_body_shadow_subcell(
                &mut self.pixel_buffer,
                rw,
                rh,
                scale,
                &alien.hull,
                -5.0,
                6.0,
                0.38,
            );
        }
        if self.in_play {
            Self::draw_body_shadow_subcell(
                &mut self.pixel_buffer,
                rw,
                rh,
                scale,
                &self.player_hull,
                -5.0,
                6.0,
                0.36,
            );
        }

        // 5. Airborne Mines/Shards, Sky Squadrons, Power Canisters, Player Ship, and Bolts
        for rock in &self.asteroids {
            draw_asteroid_or_shard_vga(&mut self.pixel_buffer, rw, rh, scale, rock);
        }
        for alien in &self.aliens {
            draw_alien_vga(&mut self.pixel_buffer, rw, rh, scale, alien);
        }
        for cube in &self.power_cubes {
            draw_power_cube_vga(&mut self.pixel_buffer, rw, rh, scale, cube);
        }
        if self.in_play {
            draw_player_raptor_vga(
                &mut self.pixel_buffer,
                rw,
                rh,
                scale,
                &self.player_hull,
                self.shield_visual_timer,
                self.thruster_visual_timer,
                self.invulnerable_steps,
            );
        }
        draw_bolts_vga(&mut self.pixel_buffer, rw, rh, scale, &self.bolts);

        // 5b. Field-Driven Fiery Explosions & Rolling Smoke (from GPU reaction-diffusion `luminescence` + `velocity`)
        if self.view_mode == FieldViewMode::CompositeArcade && has_lum {
            self.draw_field_fire_and_smoke(rw, rh, scale);
        }

        // 6. Diegetic Side Gauges, "$00000000" Bounty Counter & Ordnance Badge
        let hull_ratio =
            self.player_hull.mean_cohesion() * (self.player_intact_lobes() as f32 / 3.0);
        let cap_ratio = self.capacitor / self.max_capacitor.max(1.0);
        let wave_ratio = (self.spawn_timer as f32 / 240.0).clamp(0.0, 1.0);
        let credits = self.credits;
        Self::draw_raptor_side_gauges(
            &mut self.pixel_buffer,
            rw,
            rh,
            scale,
            hull_ratio,
            cap_ratio,
            wave_ratio,
        );
        Self::draw_raptor_bounty_ticker(&mut self.pixel_buffer, rw, rh, scale, credits);
    }

    /// Render fiery explosions and billowing dark charcoal smoke directly from the evolving
    /// reaction-diffusion `luminescence` (`L`) and hydrodynamic `velocity` (`vx, vy`) fields.
    ///
    /// - High `L > 1.75`: White-gold detonation core.
    /// - Mid `L in (1.05, 1.75]`: Turbulent orange fireball.
    /// - Rim `L in (0.65, 1.05]`: Crimson-red flame tongues.
    /// - Diffused tail `L in [0.28, 0.65]`: Rolling dark charcoal smoke clouds.
    fn draw_field_fire_and_smoke(&mut self, rw: usize, _rh: usize, scale: usize) {
        let w = self.width;
        let h = self.height;
        let inv_s = 1.0 / (scale as f32);

        for gy in 1..(h - 1) {
            let row_base = gy * w;
            for gx in 1..(w - 1) {
                let idx = row_base + gx;
                let l00 = self.solver.state.luminescence[idx];
                let l10 = self.solver.state.luminescence[idx + 1];
                let l01 = self.solver.state.luminescence[idx + w];
                let l11 = self.solver.state.luminescence[idx + w + 1];
                let l_max = l00.max(l10).max(l01).max(l11);
                if l_max < 0.28 {
                    continue;
                }

                let vx = self.solver.state.vx[idx];
                let vy = self.solver.state.vy[idx];
                let vort = (vx.abs() + vy.abs()).min(1.2) * 0.14;
                let base_py = gy * scale;
                let base_px = gx * scale;

                for sy in 0..scale {
                    let fy = (sy as f32 + 0.5) * inv_s;
                    let py = base_py + sy;
                    let row_off = py * rw + base_px;
                    for sx in 0..scale {
                        let fx = (sx as f32 + 0.5) * inv_s;
                        let px = base_px + sx;
                        let l_top = l00 * (1.0 - fx) + l10 * fx;
                        let l_bot = l01 * (1.0 - fx) + l11 * fx;
                        let l_val = l_top * (1.0 - fy) + l_bot * fy;
                        if l_val < 0.28 {
                            continue;
                        }

                        // Multi-frequency cellular turbulence so fireballs have billowing smoke rims & textured cores
                        let puff = ((px as f32 * 0.28 + py as f32 * 0.19).sin()
                            * (px as f32 * 0.17 - py as f32 * 0.31).cos())
                            * 0.38;
                        let mut h_val = (px as u32)
                            .wrapping_mul(374761393)
                            .wrapping_add((py as u32).wrapping_mul(668265263));
                        h_val = (h_val ^ (h_val >> 13)).wrapping_mul(1274126177);
                        let dither = (((h_val >> 16) & 7) as f32 - 3.5) * 0.065 + puff + vort;
                        let l_eff = l_val + dither;
                        if l_eff < 0.32 {
                            continue;
                        }

                        let p_idx = (row_off + sx) * 4;
                        if l_eff > 2.85 {
                            // Compact white-yellow hot detonation core
                            self.pixel_buffer[p_idx..p_idx + 4]
                                .copy_from_slice(&[255, 248, 184, 255]);
                        } else if l_eff > 2.05 {
                            // Bright golden-orange flame
                            self.pixel_buffer[p_idx..p_idx + 4]
                                .copy_from_slice(&[252, 168, 32, 255]);
                        } else if l_eff > 1.40 {
                            // Deep fiery orange-red flame
                            self.pixel_buffer[p_idx..p_idx + 4]
                                .copy_from_slice(&[232, 88, 20, 255]);
                        } else if l_eff > 0.92 {
                            // Dark crimson-charcoal scorched flame rim
                            self.pixel_buffer[p_idx..p_idx + 4]
                                .copy_from_slice(&[148, 32, 16, 255]);
                        } else {
                            // Wide billowing dark charcoal smoke envelope (`0.32 <= l_eff <= 0.92`)
                            let smoke_alpha = ((l_eff - 0.32) / 0.60).clamp(0.25, 0.82);
                            let keep = 1.0 - smoke_alpha;
                            self.pixel_buffer[p_idx] =
                                (self.pixel_buffer[p_idx] as f32 * keep + 22.0 * smoke_alpha) as u8;
                            self.pixel_buffer[p_idx + 1] =
                                (self.pixel_buffer[p_idx + 1] as f32 * keep + 20.0 * smoke_alpha)
                                    as u8;
                            self.pixel_buffer[p_idx + 2] =
                                (self.pixel_buffer[p_idx + 2] as f32 * keep + 24.0 * smoke_alpha)
                                    as u8;
                        }
                    }
                }
            }
        }
    }
}
