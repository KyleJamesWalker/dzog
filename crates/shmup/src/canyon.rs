//! Scrolling Ground Sector, Hydro-Canals, Industrial Platforms & Waveguide Pipelines for `crates/shmup`.
//!
//! Inspired by *Raptor: Call of the Shadows* (`ss_3ebffef`, `ss_77a1d58`, `ss_35965d0`):
//! - Lateral high-impedance cliff walls (`ObsidianAnvil`, `BismuthCrystal`, `PiezoQuartz`).
//! - Recessed `HydroCanal` water channels (`rho = 0.85, K = 0.92`) that carry visible hydrodynamic wakes.
//! - Elevated `IndustrialPlatform` tarmac blocks (`01`/`02`) with structural panel seams.
//! - Ribbed `WaveguidePipeline` conduits (`PiezoQuartz` core, `c = 1.58`) that channel shockwaves
//!   between geodesic fuel domes, biodomes, and flak turrets.

use dzog_core::{BodyMaterialSpec, BoundaryMask, MaterialField, MineralClass};
use crate::entities::{GroundTarget, GroundTargetKind};

/// Terrain substrate archetype at a scrolling ground cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundCellKind {
    CanyonWall(MineralClass),
    HydroCanal,
    WaveguidePipeline,
    IndustrialPlatform,
    TerraBasalt,
}

/// Sampled ground terrain properties at lattice coordinate `(x, y)`.
#[derive(Debug, Clone, Copy)]
pub struct GroundSample {
    pub kind: GroundCellKind,
    pub elevation: f32,
    pub is_seam_or_rib: bool,
    pub is_border_trim: bool,
    /// Secondary structural feature: recessed hangar bay, canal drop-wall, or desert foliage cluster.
    pub sub_feature: u8,
}

/// Computes deterministic East and West canyon wall thicknesses at lattice row `y` and scroll offset.
#[inline]
pub fn canyon_wall_bounds(
    y: usize,
    height: usize,
    width: usize,
    scroll_phase: f32,
) -> (usize, usize, MineralClass) {
    let wy = y as f32 - scroll_phase;
    let left_wave = (wy * 0.045).sin() * 6.0 + (wy * 0.11).cos() * 2.5;
    let right_wave = (wy * 0.042 + 1.4).cos() * 6.0 + (wy * 0.095).sin() * 2.5;

    let dist_from_bottom = height.saturating_sub(1 + y) as f32;
    let south_taper = (dist_from_bottom / (height as f32 * 0.28)).clamp(0.35, 1.0);
    let left_w = ((9.0 + left_wave) * south_taper).clamp(4.0, 18.0) as usize;
    let right_w = ((9.0 + right_wave) * south_taper).clamp(4.0, 18.0) as usize;

    let stratum = ((wy * 0.035).floor() as i32).rem_euclid(3);
    let class = match stratum {
        0 => MineralClass::ObsidianAnvil,
        1 => MineralClass::BismuthCrystal,
        _ => MineralClass::PiezoQuartz,
    };

    (left_w, width.saturating_sub(right_w), class)
}

/// Deterministic ground sector layout (hydro-canals, tarmac decks, and waveguide pipelines).
#[inline]
pub fn sample_ground_cell(
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    scroll_phase: f32,
) -> GroundSample {
    let (left_edge, right_edge, wall_class) = canyon_wall_bounds(y, height, width, scroll_phase);
    if x < left_edge || x >= right_edge {
        return GroundSample {
            kind: GroundCellKind::CanyonWall(wall_class),
            elevation: 0.42,
            is_seam_or_rib: false,
            is_border_trim: x + 1 == left_edge || x == right_edge,
            sub_feature: 0,
        };
    }

    let wy = y as f32 - scroll_phase;
    let wy_i = wy.floor() as i32;
    let wx = x as f32;
    let mid_x = width as f32 * 0.5;

    // 1. Winding central/offset Hydro-Canal + alternating West/East harbor basins (`ss_77a1d58`)
    let canal_center = mid_x + (wy * 0.024).sin() * (width as f32 * 0.095);
    let canal_half_w = 13.5 + (wy * 0.055).cos() * 3.5;
    let in_main_canal = (wx - canal_center).abs() <= canal_half_w;

    let sector_mod = wy_i.rem_euclid(116);
    let in_west_basin = (sector_mod >= 16 && sector_mod <= 40)
        && ((wx - (mid_x - width as f32 * 0.19)).abs() <= 24.0);
    let in_east_basin = (sector_mod >= 68 && sector_mod <= 90)
        && ((wx - (mid_x + width as f32 * 0.19)).abs() <= 22.0);

    if in_main_canal || in_west_basin || in_east_basin {
        let dist_edge = if in_main_canal {
            canal_half_w - (wx - canal_center).abs()
        } else if in_west_basin {
            24.0 - (wx - (mid_x - width as f32 * 0.19)).abs()
        } else {
            22.0 - (wx - (mid_x + width as f32 * 0.19)).abs()
        };
        let is_west_bank = (in_main_canal && wx < canal_center - canal_half_w + 2.4)
            || (in_west_basin && (sector_mod <= 18 || wx < mid_x - width as f32 * 0.19 - 21.5))
            || (in_east_basin && (sector_mod <= 70 || wx < mid_x + width as f32 * 0.19 - 19.5));
        return GroundSample {
            kind: GroundCellKind::HydroCanal,
            elevation: 0.15,
            is_seam_or_rib: (wy_i.rem_euclid(8) == 0) && dist_edge < 3.0,
            is_border_trim: dist_edge < 1.3,
            sub_feature: u8::from(is_west_bank),
        };
    }

    // 2. Ribbed Acoustic Waveguide Pipelines (North-South trunk lines + East-West feeder pipes)
    let left_pipe_x = (left_edge as f32 + 22.0).max(28.0);
    let right_pipe_x = (right_edge as f32 - 22.0).min(width as f32 - 28.0);
    let on_ns_pipe = (wx - left_pipe_x).abs() <= 2.2 || (wx - right_pipe_x).abs() <= 2.2;
    let ew_pipe_row = wy_i.rem_euclid(64);
    let on_ew_pipe = (ew_pipe_row >= 0 && ew_pipe_row <= 3)
        && ((wx >= left_pipe_x && wx <= canal_center - canal_half_w)
            || (wx >= canal_center + canal_half_w && wx <= right_pipe_x));

    if on_ns_pipe || on_ew_pipe {
        let is_rib = (wy_i.rem_euclid(3) == 0) || ((x as i32).rem_euclid(3) == 0 && on_ew_pipe);
        return GroundSample {
            kind: GroundCellKind::WaveguidePipeline,
            elevation: 0.46,
            is_seam_or_rib: is_rib,
            is_border_trim: false,
            sub_feature: u8::from(on_ew_pipe),
        };
    }

    // 3. Elevated Industrial Tarmac Platforms ("01" blocks in ss_77a1d58 & hangar aprons in ss_3ebffef)
    let block_phase = wy_i.rem_euclid(90);
    let in_left_deck = block_phase >= 48
        && block_phase <= 82
        && wx >= left_pipe_x + 5.0
        && wx <= canal_center - canal_half_w - 4.0;
    let in_right_deck = block_phase >= 12
        && block_phase <= 54
        && wx >= canal_center + canal_half_w + 4.0
        && wx <= right_pipe_x - 5.0;

    if in_left_deck || in_right_deck {
        let seam = (wy_i.rem_euclid(10) == 0) || ((x as i32).rem_euclid(12) == 0);
        let border = block_phase == 48
            || block_phase == 82
            || block_phase == 12
            || block_phase == 54;
        let in_recess_bay = !seam
            && !border
            && (wy_i.rem_euclid(10) >= 2 && wy_i.rem_euclid(10) <= 7)
            && ((x as i32).rem_euclid(12) >= 2 && (x as i32).rem_euclid(12) <= 9)
            && ((wy_i / 10 + (x as i32) / 12) % 3 == 0);
        return GroundSample {
            kind: GroundCellKind::IndustrialPlatform,
            elevation: 0.54,
            is_seam_or_rib: seam,
            is_border_trim: border,
            sub_feature: u8::from(in_recess_bay),
        };
    }

    // 4. Open Terra-Basalt ground plane with geological strata & desert foliage clusters (`ss_3ebffef`)
    let regolith_seam = ((wx * 0.19 + wy * 0.13).sin() * (wx * 0.07 - wy * 0.21).cos()) > 0.76;
    let shrub_cell_x = (x as i32) / 7;
    let shrub_cell_y = wy_i.div_euclid(7);
    let shrub_hash =
        (shrub_cell_x as u32).wrapping_mul(73856093) ^ (shrub_cell_y as u32).wrapping_mul(19349663);
    let in_foliage = if (shrub_hash & 0x1F) == 0 {
        let local_x = (x as i32).rem_euclid(7) as f32 - 3.0;
        let local_y = wy_i.rem_euclid(7) as f32 - 3.0;
        local_x * local_x + local_y * local_y <= 3.8
    } else {
        false
    };

    GroundSample {
        kind: GroundCellKind::TerraBasalt,
        elevation: 0.42,
        is_seam_or_rib: regolith_seam,
        is_border_trim: false,
        sub_feature: u8::from(in_foliage),
    }
}

/// Evaluate the 3D heightmap `h in [0.0, 0.90]` and unit surface normal `(nx, ny, nz)` of a
/// `GroundTarget` building at world coordinate `(wx, wy)`. Returns `None` if outside the intact hull.
#[inline]
pub fn sample_building_height_and_normal(
    gt: &GroundTarget,
    wx: f32,
    wy: f32,
) -> Option<(f32, f32, f32, f32)> {
    let body = &gt.hull;
    let dx = wx - body.x;
    let dy = wy - body.y;
    if dx * dx + dy * dy > (body.radius + 1.0).powi(2) {
        return None;
    }
    let center = (body.local_size as f32 - 1.0) * 0.5;
    let coh = body.sample_local_cohesion(dx + center, dy + center);
    if coh < 0.15 {
        return None;
    }

    let inv_r = 1.0 / body.radius.max(1.0);
    let nx_raw = dx * inv_r;
    let ny_raw = dy * inv_r;
    let r_norm = (nx_raw * nx_raw + ny_raw * ny_raw).sqrt().clamp(0.0, 1.0);

    let (h, sx, sy, sz) = match gt.kind {
        GroundTargetKind::GeodesicFuelDome => {
            let dome_z = (1.0 - r_norm * r_norm).max(0.08).sqrt();
            (0.72 + 0.16 * dome_z, nx_raw, ny_raw, dome_z)
        }
        GroundTargetKind::BiodomeReactor => {
            if r_norm > 0.76 {
                (0.76, nx_raw * 1.2, ny_raw * 1.2, 0.45)
            } else {
                let inner_r = (r_norm / 0.76).clamp(0.0, 1.0);
                let dome_z = (1.0 - inner_r * inner_r).max(0.08).sqrt();
                (0.76 + 0.14 * dome_z, nx_raw, ny_raw, dome_z)
            }
        }
        GroundTargetKind::FlakTurret => {
            let dome_z = (1.0 - r_norm * r_norm).max(0.10).sqrt();
            (0.68 + 0.14 * dome_z, nx_raw, ny_raw, dome_z)
        }
        GroundTargetKind::CanalPatrolBoat => {
            let in_bridge = nx_raw.abs() < 0.38 && ny_raw.abs() < 0.38;
            let h = if in_bridge { 0.48 } else { 0.34 };
            (h, nx_raw * 0.6, ny_raw * 0.6, 0.85)
        }
    };

    let len = (sx * sx + sy * sy + sz * sz).sqrt().max(0.001);
    Some((h, sx / len, sy / len, sz / len))
}

/// Stamp the lateral metamaterial canyon walls, hydro-canals, waveguide pipelines, and
/// top/bottom absorbing horizon sponges into `material` and `boundary`.
pub fn stamp_canyon_into_lattice(
    material: &mut MaterialField,
    boundary: &mut BoundaryMask,
    scroll_phase: f32,
) {
    let w = material.config.width;
    let h = material.config.height;

    for y in 0..h {
        let (left_edge, right_edge, _) = canyon_wall_bounds(y, h, w, scroll_phase);

        let horizon_sponge = if y < 6 {
            (6 - y) as f32 * 0.045
        } else if y + 6 >= h {
            (y + 6 - h + 1) as f32 * 0.045
        } else {
            0.0
        };

        for x in 0..w {
            let idx = material.config.index(x, y);
            let sample = sample_ground_cell(x, y, w, h, scroll_phase);
            if idx < material.elevation.len() {
                // Keep terrain baseline at 0.0 in shadow_horizon so only 3D buildings cast horizon shadows
                material.elevation[idx] = 0.0;
            }

            match sample.kind {
                GroundCellKind::CanyonWall(class) => {
                    let spec = BodyMaterialSpec::for_class(class);
                    material.density[idx] = spec.rho;
                    material.bulk_modulus[idx] = spec.bulk_modulus;
                    material.damping[idx] = spec.damping + horizon_sponge;
                    material.cohesion[idx] = 0.95;
                    material.yield_pressure[idx] = spec.yield_pressure;

                    let inner_rim = (x + 2 >= left_edge && x < left_edge)
                        || (x >= right_edge && x < right_edge + 2);
                    if !inner_rim {
                        boundary.wall_east[idx] = true;
                        boundary.wall_north[idx] = true;
                    }
                }
                GroundCellKind::HydroCanal => {
                    // Low-impedance water channel: carries crisp acoustic & thruster wakes
                    material.density[idx] = 0.85;
                    material.bulk_modulus[idx] = 0.92;
                    material.damping[idx] = (0.011f32).max(horizon_sponge);
                    material.cohesion[idx] = 0.0;
                    material.yield_pressure[idx] = 8.0;
                }
                GroundCellKind::WaveguidePipeline => {
                    // Fast-phonon Piezo conduit that guides shockwaves between base structures
                    material.density[idx] = 0.74;
                    material.bulk_modulus[idx] = 1.82;
                    material.damping[idx] = (0.004f32).max(horizon_sponge);
                    material.cohesion[idx] = 0.75;
                    material.yield_pressure[idx] = 3.5;
                }
                GroundCellKind::IndustrialPlatform => {
                    material.density[idx] = 1.28;
                    material.bulk_modulus[idx] = 1.42;
                    material.damping[idx] = (0.015f32).max(horizon_sponge);
                    material.cohesion[idx] = 0.35;
                    material.yield_pressure[idx] = 6.0;
                }
                GroundCellKind::TerraBasalt => {
                    material.density[idx] = 1.0;
                    material.bulk_modulus[idx] = 1.0;
                    material.damping[idx] = (0.018f32).max(horizon_sponge);
                    material.cohesion[idx] = 0.0;
                    material.yield_pressure[idx] = 10.0;
                }
            }
        }
    }
}
