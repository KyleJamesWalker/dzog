//! Scrolling Metamaterial Canyon Waveguide for `crates/shmup`.
//!
//! Stamps heterogeneous lateral canyon walls (`ObsidianAnvil` high-impedance reflectors and
//! `PiezoQuartz` fast-phonon ledges) into the lattice as the vertical wave-stream scrolls.
//! Firing near canyon walls creates grazing Fresnel reflections and standing-wave pockets.

use dzog_core::{BodyMaterialSpec, BoundaryMask, MaterialField, MineralClass};

/// Computes deterministic East and West canyon wall thicknesses at lattice row `y` and scroll offset.
#[inline]
pub fn canyon_wall_bounds(y: usize, height: usize, width: usize, scroll_phase: f32) -> (usize, usize, MineralClass) {
    // Subtract scroll_phase so canyon geological features move downward (+Y) with the stream
    let wy = y as f32 - scroll_phase;
    // Multi-harmonic canyon wall undulation
    let left_wave = (wy * 0.045).sin() * 7.5 + (wy * 0.11).cos() * 3.5;
    let right_wave = (wy * 0.042 + 1.4).cos() * 7.5 + (wy * 0.095).sin() * 3.5;

    // Taper canyon slightly near screen-bottom (high y) so player spawn zone has maneuvering room
    let dist_from_bottom = height.saturating_sub(1 + y) as f32;
    let south_taper = (dist_from_bottom / (height as f32 * 0.28)).clamp(0.35, 1.0);
    let left_w = ((10.0 + left_wave) * south_taper).clamp(4.0, 22.0) as usize;
    let right_w = ((10.0 + right_wave) * south_taper).clamp(4.0, 22.0) as usize;

    // Alternate mineralogical strata between ObsidianAnvil reflectors and Bismuth/Piezo ledges
    let stratum = ((wy * 0.035).floor() as i32).rem_euclid(3);
    let class = match stratum {
        0 => MineralClass::ObsidianAnvil,
        1 => MineralClass::BismuthCrystal,
        _ => MineralClass::PiezoQuartz,
    };

    (left_w, width.saturating_sub(right_w), class)
}

/// Stamp the lateral metamaterial canyon walls and top/bottom absorbing horizon sponges
/// into `material` and `boundary`.
pub fn stamp_canyon_into_lattice(
    material: &mut MaterialField,
    boundary: &mut BoundaryMask,
    scroll_phase: f32,
) {
    let w = material.config.width;
    let h = material.config.height;

    for y in 0..h {
        let (left_edge, right_edge, class) = canyon_wall_bounds(y, h, w, scroll_phase);
        let spec = BodyMaterialSpec::for_class(class);

        // Top and bottom open horizon absorbing sponge (prevents artificial North/South box reflections)
        let horizon_sponge = if y < 6 {
            (6 - y) as f32 * 0.045
        } else if y + 6 >= h {
            (y + 6 - h + 1) as f32 * 0.045
        } else {
            0.0
        };

        for x in 0..w {
            let idx = material.config.index(x, y);
            let in_canyon_wall = x < left_edge || x >= right_edge;
            if in_canyon_wall {
                material.density[idx] = spec.rho;
                material.bulk_modulus[idx] = spec.bulk_modulus;
                material.damping[idx] = spec.damping + horizon_sponge;
                material.cohesion[idx] = 0.95;
                material.yield_pressure[idx] = spec.yield_pressure;

                // Exposed inner canyon faces reflect acoustic waves via Fresnel impedance + rigid wall mask
                let inner_rim = (x + 2 >= left_edge && x < left_edge)
                    || (x >= right_edge && x < right_edge + 2);
                if !inner_rim {
                    boundary.wall_east[idx] = true;
                    boundary.wall_north[idx] = true;
                }
            } else if horizon_sponge > 0.0 {
                material.damping[idx] = material.damping[idx].max(horizon_sponge);
            }
        }
    }
}
