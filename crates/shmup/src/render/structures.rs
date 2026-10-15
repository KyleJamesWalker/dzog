//! 1994 VGA Ground Structures, Mines, Pickups & Ordnance (`crates/shmup/src/render/structures.rs`).

use dzog_core::{CohesiveBody, MineralClass};
use crate::entities::{GroundTarget, GroundTargetKind, PowerCube, ProjectileKind, SolitonBolt};
use super::{check_subpixel_edge, quantize_vga, write_px};

/// Render a ground structure (`GeodesicFuelDome`, `BiodomeReactor`, `FlakTurret`, `CanalPatrolBoat`)
/// at `scale` sub-cell resolution with 1994 *Raptor* VGA architectural detailing.
pub fn draw_ground_target_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    gt: &GroundTarget,
    player_pos: (f32, f32),
) {
    let body = &gt.hull;
    let s_f = scale as f32;
    let step = 1.0 / s_f;
    let r_sub = ((body.radius + 1.5) * s_f).ceil() as isize;
    let cx_sub = (body.x * s_f).round() as isize;
    let cy_sub = (body.y * s_f).round() as isize;
    let center = (body.local_size as f32 - 1.0) * 0.5;
    let inv_r = 1.0 / body.radius.max(1.0);

    for oy in -r_sub..=r_sub {
        for ox in -r_sub..=r_sub {
            let px = cx_sub + ox;
            let py = cy_sub + oy;
            if px < 12 || px >= rw as isize - 12 || py < 0 || py >= rh as isize {
                continue;
            }
            let wx = (px as f32 + 0.5) * step;
            let wy = (py as f32 + 0.5) * step;
            let dx = wx - body.x;
            let dy = wy - body.y;
            let lx = dx + center;
            let ly = dy + center;
            let coh = body.sample_local_cohesion(lx, ly);
            if coh < 0.15 {
                continue;
            }

            let (is_edge, _is_fracture_edge) = check_subpixel_edge(body, lx, ly, step * 1.15);
            if is_edge {
                write_px(pixel_buffer, rw, px, py, [20, 22, 28]);
                continue;
            }

            let nx = dx * inv_r;
            let ny = dy * inv_r;
            let r_norm = (nx * nx + ny * ny).sqrt().clamp(0.0, 1.0);
            let nz = (1.0 - r_norm * r_norm * 0.88).max(0.08).sqrt();
            let spec = (-nx * 0.48 - ny * 0.52 + nz * 0.68).clamp(0.18, 1.28);
            let crack = if coh < 0.68 { (0.68 - coh) * 2.6 } else { 0.0 };

            let (mut r, mut g, mut b) = match gt.kind {
                GroundTargetKind::GeodesicFuelDome => {
                    let cardinal_lug = r_norm > 0.76 && (nx.abs() < 0.20 || ny.abs() < 0.20);
                    let diag1 = ((dx + dy).rem_euclid(2.6) - 1.3).abs() < 0.22;
                    let diag2 = ((dx - dy).rem_euclid(2.6) - 1.3).abs() < 0.22;
                    let ortho = (dx.abs().rem_euclid(2.6) < 0.20)
                        || (dy.abs().rem_euclid(2.6) < 0.20);
                    if cardinal_lug {
                        (0.68 * spec, 0.70 * spec, 0.75 * spec)
                    } else if diag1 || diag2 || ortho {
                        (0.26 * spec, 0.28 * spec, 0.32 * spec)
                    } else {
                        (0.64 * spec, 0.67 * spec, 0.72 * spec)
                    }
                }
                GroundTargetKind::BiodomeReactor => {
                    let in_collar = r_norm > 0.76;
                    let glass_grid = (dx.abs().rem_euclid(2.8) < 0.28)
                        || (dy.abs().rem_euclid(2.8) < 0.28);
                    if in_collar {
                        (0.46 * spec, 0.48 * spec, 0.52 * spec)
                    } else if glass_grid {
                        (0.66 * spec, 0.72 * spec, 0.70 * spec)
                    } else {
                        let canopy_patch = ((dx * 1.1).sin() * (dy * 1.1).cos()) * 0.14;
                        (
                            (0.08 + canopy_patch.max(0.0)) * spec,
                            (0.46 + canopy_patch) * spec,
                            (0.18 + canopy_patch * 0.5) * spec,
                        )
                    }
                }
                GroundTargetKind::FlakTurret => {
                    let aim_dx = player_pos.0 - body.x;
                    let aim_dy = (player_pos.1 - body.y).max(1.0);
                    let aim_len = (aim_dx * aim_dx + aim_dy * aim_dy).sqrt().max(1.0);
                    let ux = aim_dx / aim_len;
                    let uy = aim_dy / aim_len;
                    let proj = dx * ux + dy * uy;
                    let perp = (dx * (-uy) + dy * ux).abs();
                    let on_barrel =
                        proj > 0.0 && proj <= body.radius * 0.95 && (perp - 1.15).abs() < 0.45;
                    let on_muzzle =
                        proj > body.radius * 0.55 && (perp - 1.15).abs() < 0.55;
                    if on_muzzle {
                        (0.98, 0.68, 0.14)
                    } else if on_barrel {
                        (0.25, 0.28, 0.32)
                    } else {
                        (0.44 * spec, 0.42 * spec, 0.47 * spec)
                    }
                }
                GroundTargetKind::CanalPatrolBoat => {
                    let in_cabin = nx.abs() < 0.35 && ny > -0.35 && ny < 0.15;
                    let in_cargo_net = nx.abs() < 0.48 && ny <= -0.25;
                    if in_cabin {
                        (0.70 * spec, 0.76 * spec, 0.82 * spec)
                    } else if in_cargo_net {
                        let mesh = ((dx * 2.5).sin() * (dy * 2.5).cos()).abs() > 0.35;
                        if mesh {
                            (0.18 * spec, 0.44 * spec, 0.22 * spec)
                        } else {
                            (0.12 * spec, 0.26 * spec, 0.14 * spec)
                        }
                    } else {
                        (0.48 * spec, 0.50 * spec, 0.54 * spec)
                    }
                }
            };

            // Damaged building fractures render as charred dark structural fissures (`ss_7541140`)
            // rather than neon orange bands
            if crack > 0.0 {
                if coh < 0.36 {
                    let spark = ((px + py * 3) % 11 == 0) && coh < 0.28;
                    if spark {
                        r = 0.85;
                        g = 0.38;
                        b = 0.12;
                    } else {
                        r *= 0.25;
                        g *= 0.24;
                        b *= 0.26;
                    }
                } else {
                    r *= 0.52;
                    g *= 0.50;
                    b *= 0.52;
                }
            }

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

/// Render an armored aerial mine or severed hull shard at `scale` sub-cell resolution.
pub fn draw_asteroid_or_shard_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    body: &CohesiveBody,
) {
    let s_f = scale as f32;
    let step = 1.0 / s_f;
    let r_sub = ((body.radius + 1.5) * s_f).ceil() as isize;
    let cx_sub = (body.x * s_f).round() as isize;
    let cy_sub = (body.y * s_f).round() as isize;
    let cos_t = body.theta.cos();
    let sin_t = body.theta.sin();
    let center = (body.local_size as f32 - 1.0) * 0.5;

    let tint = match body.material.class {
        MineralClass::PiezoQuartz => (0.50, 0.56, 0.64),
        MineralClass::ObsidianAnvil => (0.38, 0.42, 0.48),
        MineralClass::BismuthCrystal => (0.46, 0.44, 0.50),
        MineralClass::AlienResonator => (0.42, 0.52, 0.48),
    };

    for oy in -r_sub..=r_sub {
        for ox in -r_sub..=r_sub {
            let px = cx_sub + ox;
            let py = cy_sub + oy;
            if px < 0 || px >= rw as isize || py < 0 || py >= rh as isize {
                continue;
            }
            let wx = (px as f32 + 0.5) * step;
            let wy = (py as f32 + 0.5) * step;
            let dx = wx - body.x;
            let dy = wy - body.y;
            let lx = dx * cos_t + dy * sin_t + center;
            let ly = -dx * sin_t + dy * cos_t + center;
            let coh = body.sample_local_cohesion(lx, ly);
            if coh < 0.15 {
                continue;
            }

            let (is_edge, is_fracture_edge) = check_subpixel_edge(body, lx, ly, step * 1.15);
            if is_edge {
                let col = if is_fracture_edge {
                    [255, 160, 50]
                } else {
                    [22, 26, 34]
                };
                write_px(pixel_buffer, rw, px, py, col);
                continue;
            }

            let is_seam = ((lx * 1.8).floor() as i32 % 4 == 0)
                || ((ly * 1.8).floor() as i32 % 4 == 0);
            let seam = if is_seam { -0.07 } else { 0.03 };
            let light = (0.72 - dx * 0.04 - dy * 0.05).clamp(0.35, 1.15);
            let crack = if coh < 0.65 { (0.65 - coh) * 2.2 } else { 0.0 };

            let r = (tint.0 + seam) * light + crack * 0.85;
            let g = (tint.1 + seam) * light + crack * 0.35;
            let b = (tint.2 + seam) * light + crack * 0.12;

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

/// Render an Annealing Power Cube as a compact framed VGA supply canister.
pub fn draw_power_cube_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    cube: &PowerCube,
) {
    let s_f = scale as f32;
    let cx = (cube.body.x * s_f).round() as isize;
    let cy = (cube.body.y * s_f).round() as isize;
    let half = (scale as isize) + 2;

    for oy in -half..=half {
        for ox in -half..=half {
            let px = cx + ox;
            let py = cy + oy;
            if px < 0 || px >= rw as isize || py < 0 || py >= rh as isize {
                continue;
            }
            let ax = ox.abs();
            let ay = oy.abs();
            if ax == half && ay == half {
                continue;
            }
            let is_outer_rim = ax == half || ay == half || (ax + ay == half * 2 - 1);
            let is_gold_frame = ax == half - 1 || ay == half - 1;
            let is_glint = ox <= 0 && oy <= 0 && (ax + ay <= 2);

            let col = if is_outer_rim {
                [20, 26, 32]
            } else if is_gold_frame {
                [232, 176, 48]
            } else if is_glint {
                [224, 255, 240]
            } else {
                [40, 224, 136]
            };
            write_px(pixel_buffer, rw, px, py, col);
        }
    }
}

/// Render player and enemy projectiles with *Raptor* VGA contrast outlines and hot cores (`ss_3ebffef`).
pub fn draw_bolts_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    bolts: &[SolitonBolt],
) {
    let s_f = scale as f32;
    for bolt in bolts {
        let bx = (bolt.x * s_f).round() as isize;
        let by = (bolt.y * s_f).round() as isize;

        if bolt.kind == ProjectileKind::AirToGroundMissile {
            let half_w = scale as isize;
            let half_h = (scale as isize * 3) + 1;
            for oy in -half_h..=(half_h + scale as isize) {
                for ox in -(half_w + 1)..=(half_w + 1) {
                    let px = bx + ox;
                    let py = by + oy;
                    if px < 0 || px >= rw as isize || py < 0 || py >= rh as isize {
                        continue;
                    }
                    let ax = ox.abs();
                    if oy > half_h {
                        if ax <= half_w - 1 {
                            let col = if ax == 0 {
                                [220, 250, 255]
                            } else {
                                [56, 168, 248]
                            };
                            write_px(pixel_buffer, rw, px, py, col);
                        }
                        continue;
                    }
                    let in_fins = oy >= half_h - 3 && ax == half_w + 1;
                    if !in_fins && ax > half_w {
                        continue;
                    }
                    let col = if in_fins || ax == half_w || oy == -half_h {
                        [24, 28, 36]
                    } else if oy < -half_h + 4 {
                        [224, 48, 40]
                    } else if ox <= 0 {
                        [208, 216, 224]
                    } else {
                        [136, 144, 156]
                    };
                    write_px(pixel_buffer, rw, px, py, col);
                }
            }
        } else if bolt.from_player {
            let half_w = if bolt.overcharged {
                scale as isize + 1
            } else {
                scale as isize
            };
            let half_h = (scale as isize * 2) + 1;
            for oy in -half_h..=half_h {
                for ox in -half_w..=half_w {
                    let px = bx + ox;
                    let py = by + oy;
                    if px < 0 || px >= rw as isize || py < 0 || py >= rh as isize {
                        continue;
                    }
                    let norm = (ox as f32 / half_w as f32).powi(2)
                        + (oy as f32 / half_h as f32).powi(2);
                    if norm > 1.05 {
                        continue;
                    }
                    let col = if norm > 0.72 {
                        [24, 42, 64]
                    } else if norm > 0.28 {
                        if bolt.overcharged {
                            [255, 200, 56]
                        } else {
                            [72, 224, 248]
                        }
                    } else {
                        [255, 255, 240]
                    };
                    write_px(pixel_buffer, rw, px, py, col);
                }
            }
        } else {
            let r_px = (scale as isize) + 2;
            for oy in -r_px..=r_px {
                for ox in -r_px..=r_px {
                    let px = bx + ox;
                    let py = by + oy;
                    if px < 0 || px >= rw as isize || py < 0 || py >= rh as isize {
                        continue;
                    }
                    let d2 = (ox * ox + oy * oy) as f32;
                    let r2 = (r_px * r_px) as f32;
                    if d2 > r2 {
                        continue;
                    }
                    let col = if d2 > r2 * 0.68 {
                        [48, 20, 8]
                    } else if d2 > r2 * 0.22 {
                        [248, 160, 24]
                    } else {
                        [255, 248, 184]
                    };
                    write_px(pixel_buffer, rw, px, py, col);
                }
            }
        }
    }
}
