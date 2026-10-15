//! 1994 VGA Ship & Squadron Rendering (`crates/shmup/src/render/ships.rs`).

use dzog_core::CohesiveBody;
use crate::entities::{AlienRole, AlienVessel};
use super::{check_subpixel_edge, quantize_vga, write_px};

/// Render the player's Tri-Lobe *Raptor* Interceptor at `scale` sub-cell resolution (`~40x40` px).
pub fn draw_player_raptor_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    ship: &CohesiveBody,
    shield_timer: u8,
    thruster_timer: u8,
    invuln_steps: u16,
) {
    let s_f = scale as f32;
    let step = 1.0 / s_f;
    let r_sub = ((ship.radius + 1.5) * s_f).ceil() as isize;
    let cx_sub = (ship.x * s_f).round() as isize;
    let cy_sub = (ship.y * s_f).round() as isize;
    let cos_t = ship.theta.cos();
    let sin_t = ship.theta.sin();
    let center = (ship.local_size as f32 - 1.0) * 0.5;
    let inv_r = 1.0 / ship.radius.max(1.0);

    for oy in -r_sub..=r_sub {
        for ox in -r_sub..=r_sub {
            let px = cx_sub + ox;
            let py = cy_sub + oy;
            if px < 0 || px >= rw as isize || py < 0 || py >= rh as isize {
                continue;
            }
            let wx = (px as f32 + 0.5) * step;
            let wy = (py as f32 + 0.5) * step;
            let dx = wx - ship.x;
            let dy = wy - ship.y;
            let lx = dx * cos_t + dy * sin_t + center;
            let ly = -dx * sin_t + dy * cos_t + center;
            let coh = ship.sample_local_cohesion(lx, ly);
            if coh < 0.15 {
                continue;
            }

            let (is_edge, is_fracture_edge) = check_subpixel_edge(ship, lx, ly, step * 1.15);
            if is_edge {
                let edge_col = if shield_timer > 0 {
                    [110, 245, 255]
                } else if is_fracture_edge {
                    [255, 155, 45]
                } else {
                    [20, 24, 34]
                };
                write_px(pixel_buffer, rw, px, py, edge_col);
                continue;
            }

            let nx = (lx - center) * inv_r;
            let ny = (ly - center) * inv_r;
            let abs_x = nx.abs();
            let light = (0.72 - nx * 0.28 - ny * 0.22).clamp(0.35, 1.18);
            let crack = if coh < 0.68 { (0.68 - coh) * 2.5 } else { 0.0 };

            let in_canopy = abs_x < 0.17 * (1.0 - (ny + 0.28).abs() * 1.8).max(0.2)
                && ny >= -0.58
                && ny <= -0.04;
            let stripe_center = -0.02 + (abs_x - 0.32).max(0.0) * 0.52;
            let in_red_stripe =
                abs_x >= 0.32 && abs_x <= 0.88 && (ny - stripe_center).abs() <= 0.13;
            let in_wing_gun = (abs_x - 0.60).abs() < 0.06 && ny < -0.10 && ny >= -0.46;
            let in_nozzle = (abs_x - 0.18).abs() < 0.09 && ny > 0.48;
            let is_spine = abs_x < 0.06 && ny >= -0.78 && ny <= 0.48;
            let is_panel_seam = ((lx * 2.2).floor() as i32 % 5 == 0)
                || ((ly * 2.2).floor() as i32 % 6 == 0);

            let (mut r, mut g, mut b) = if in_canopy {
                let glint = nx < -0.02 && ny < -0.25 && ny > -0.46;
                if glint {
                    (0.82, 0.96, 1.0)
                } else {
                    (0.11 * light, 0.22 * light, 0.36 * light)
                }
            } else if in_red_stripe {
                (0.86 * light, 0.16 * light, 0.18 * light)
            } else if in_wing_gun {
                (0.28 * light, 0.32 * light, 0.38 * light)
            } else if in_nozzle {
                if ny > 0.62 {
                    (0.35, 0.85, 1.0)
                } else {
                    (0.22, 0.25, 0.30)
                }
            } else if is_spine {
                (0.82 * light, 0.86 * light, 0.92 * light)
            } else {
                let seam_mod = if is_panel_seam { -0.08 } else { 0.0 };
                (
                    (0.56 + seam_mod) * light,
                    (0.60 + seam_mod) * light,
                    (0.66 + seam_mod) * light,
                )
            };

            if invuln_steps % 4 >= 2 {
                r += 0.18;
                g += 0.22;
                b += 0.25;
            }
            if crack > 0.0 {
                r = r * (1.0 - crack * 0.5) + crack * 0.98;
                g = g * (1.0 - crack * 0.5) + crack * 0.45;
                b = b * (1.0 - crack * 0.5) + crack * 0.15;
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

    let plume_len = if thruster_timer > 0 {
        (5 * scale) as isize
    } else {
        (3 * scale) as isize
    };
    for side in [-1isize, 1isize] {
        let base_x = cx_sub + side * (scale as isize * 3 / 2);
        let base_y = cy_sub + (scale as isize * 4);
        for py_off in 0..plume_len {
            let frac = 1.0 - (py_off as f32 / plume_len as f32);
            let half_w = ((frac * scale as f32 * 0.85) as isize).max(1);
            for px_off in -half_w..=half_w {
                let px = base_x + px_off;
                let py = base_y + py_off;
                if px >= 0 && px < rw as isize && py >= 0 && py < rh as isize {
                    let col = if px_off == 0 && py_off < plume_len / 2 {
                        [235, 250, 255]
                    } else if thruster_timer > 0 {
                        [255, 180, 65]
                    } else {
                        [75, 185, 245]
                    };
                    write_px(pixel_buffer, rw, px, py, col);
                }
            }
        }
    }
}

/// Render an airborne enemy (`TwinGunship`, `ChladniMatriarch`, `SectorDreadnought`, `BohmianSwarmDrone`)
/// with *Raptor* VGA armor plating, glowing weapon ports, and progressive fracture fissures.
pub fn draw_alien_vga(
    pixel_buffer: &mut [u8],
    rw: usize,
    rh: usize,
    scale: usize,
    alien: &AlienVessel,
) {
    let body = &alien.hull;
    let s_f = scale as f32;
    let step = 1.0 / s_f;
    let r_sub = ((body.radius + 1.5) * s_f).ceil() as isize;
    let cx_sub = (body.x * s_f).round() as isize;
    let cy_sub = (body.y * s_f).round() as isize;
    let cos_t = body.theta.cos();
    let sin_t = body.theta.sin();
    let center = (body.local_size as f32 - 1.0) * 0.5;
    let inv_r = 1.0 / body.radius.max(1.0);

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
                    [255, 140, 35]
                } else {
                    [18, 20, 28]
                };
                write_px(pixel_buffer, rw, px, py, col);
                continue;
            }

            let nx = (lx - center) * inv_r;
            let ny = (ly - center) * inv_r;
            let r_norm = (nx * nx + ny * ny).sqrt().clamp(0.0, 1.0);
            let light = (0.76 - dy * inv_r * 0.25 - dx * inv_r * 0.25).clamp(0.32, 1.18);
            let crack = if coh < 0.72 { (0.72 - coh) * 2.6 } else { 0.0 };

            let (mut r, mut g, mut b) = match alien.role {
                AlienRole::TwinGunship | AlienRole::SectorDreadnought => {
                    let is_boss = alien.role == AlienRole::SectorDreadnought;
                    let abs_y = ny.abs();
                    let in_orange_port =
                        nx > 0.28 && nx < 0.62 && (abs_y - 0.66).abs() < 0.16;
                    let in_grille = nx < 0.05
                        && nx > -0.55
                        && abs_y > 0.48
                        && ((nx * 14.0).sin() > 0.25);
                    let in_cockpit = nx > 0.10 && nx < 0.55 && abs_y < 0.14;
                    let in_exhaust = nx < -0.62 && (abs_y > 0.48 || abs_y < 0.16);
                    let in_teal_trim = (abs_y - 0.46).abs() < 0.07;

                    if in_orange_port {
                        (0.98, 0.68, 0.12)
                    } else if in_exhaust {
                        (0.85, 0.72, 0.52)
                    } else if in_cockpit {
                        if is_boss {
                            (0.88, 0.20, 0.24)
                        } else {
                            (0.16, 0.28, 0.38)
                        }
                    } else if in_teal_trim {
                        if is_boss {
                            (0.88 * light, 0.64 * light, 0.18 * light)
                        } else {
                            (0.22 * light, 0.68 * light, 0.78 * light)
                        }
                    } else if in_grille {
                        (0.20 * light, 0.22 * light, 0.26 * light)
                    } else {
                        let brown_steel = if nx < 0.0 { 0.06 } else { 0.0 };
                        (
                            (0.46 + brown_steel) * light,
                            0.42 * light,
                            0.45 * light,
                        )
                    }
                }
                AlienRole::ChladniMatriarch => {
                    let in_core = r_norm < 0.30;
                    let in_ring = (r_norm - 0.52).abs() < 0.07;
                    let seam = ((lx * 2.0).floor() as i32 % 4 == 0)
                        || ((ly * 2.0).floor() as i32 % 4 == 0);
                    if in_core {
                        let pulse = (alien.carrier_phase.sin() * 0.2 + 0.8).clamp(0.5, 1.0);
                        (0.95 * pulse, 0.45 * pulse, 0.88 * pulse)
                    } else if in_ring {
                        (0.88 * light, 0.68 * light, 0.28 * light)
                    } else {
                        let s = if seam { -0.08 } else { 0.0 };
                        (
                            (0.54 + s) * light,
                            (0.32 + s) * light,
                            (0.52 + s) * light,
                        )
                    }
                }
                AlienRole::BohmianSwarmDrone { .. } => {
                    let in_eye = r_norm < 0.26;
                    let in_cross = nx.abs() < 0.14 || ny.abs() < 0.14;
                    if in_eye {
                        (0.98, 0.32, 0.18)
                    } else if in_cross {
                        let tick = ((nx.abs() + ny.abs()) * 9.0).fract() > 0.55;
                        if tick {
                            (0.92, 0.68, 0.16)
                        } else {
                            (0.18, 0.20, 0.25)
                        }
                    } else {
                        (0.42 * light, 0.45 * light, 0.50 * light)
                    }
                }
                AlienRole::AnnealingHunter => {
                    let in_core = r_norm < 0.28;
                    if in_core {
                        (0.98, 0.82, 0.22)
                    } else {
                        (0.68 * light, 0.24 * light, 0.30 * light)
                    }
                }
            };

            // Sub-cell jagged lightning-crack fissure inside the fractured cohesion zone (`coh < 0.72`)
            if crack > 0.0 {
                let branch1 = ((lx * 3.8 + (ly * 2.7).sin() * 1.35).sin()).abs();
                let branch2 = ((ly * 4.1 - (lx * 2.9).cos() * 1.25).sin()).abs();
                let vein_dist = branch1.min(branch2);
                if coh < 0.52 && vein_dist < 0.16 {
                    // White-gold hot fracture core (1px wide)
                    r = 1.0;
                    g = 0.90;
                    b = 0.38;
                } else if coh < 0.62 && vein_dist < 0.34 {
                    // Molten orange-red crack seam
                    r = 0.96;
                    g = 0.36;
                    b = 0.08;
                } else if vein_dist < 0.58 {
                    // Charred black carbon-soot crack lip
                    r *= 0.22;
                    g *= 0.20;
                    b *= 0.22;
                } else {
                    // Slightly darkened stressed armor plate around the crack
                    r *= 0.72;
                    g *= 0.70;
                    b *= 0.72;
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
