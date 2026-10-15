//! Free-rotating cohesive bodies, rotating Yee-grid rasterization, contour force/torque
//! integration, and connected-component topological cleaving.
//!
//! Operationalizes `philosophy.md` §A Living Ecology of Actors:
//! - Bodies are not isolated rigid-body polygons; they are rotating local material grids
//!   coupled bidirectionally to the staggered `(p, vx, vy, Q)` field.
//! - Internal fractures open semi-permeable acoustic waveguides (`C < 0.55`) that channel
//!   phonons through the body.
//! - When cleavage fissures sever a body's connected cell topology, it splits into independent
//!   tumbling child fragments conserving linear and angular momentum `(P, L_z)`.

use std::collections::VecDeque;
use std::f32::consts::TAU;
use serde::{Deserialize, Serialize};
use crate::boundary::BoundaryMask;
use crate::grid::AcousticState;
use crate::material::MaterialField;

/// Mineralogical / structural material archetype for a cohesive body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MineralClass {
    /// Fast phonon speed (c = 1.48), brittle hexagonal cleavage, radiates piezo shock rings.
    PiezoQuartz,
    /// High acoustic impedance (Z = 2.02), reflects glancing standard bolts, heavy recoil torque.
    ObsidianAnvil,
    /// Stepped metamaterial crystal (c = 1.10), channels and lenses internal wave energy.
    BismuthCrystal,
    /// Alien resonant saucer hull.
    AlienResonator,
}

/// Physical material parameters for a cohesive body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BodyMaterialSpec {
    pub class: MineralClass,
    pub rho: f32,
    pub bulk_modulus: f32,
    pub damping: f32,
    pub yield_pressure: f32,
}

impl BodyMaterialSpec {
    pub const fn for_class(class: MineralClass) -> Self {
        match class {
            MineralClass::PiezoQuartz => Self {
                class,
                rho: 0.75,
                bulk_modulus: 1.65,
                damping: 0.002,
                yield_pressure: 1.75,
            },
            MineralClass::ObsidianAnvil => Self {
                class,
                rho: 2.00,
                bulk_modulus: 2.05,
                damping: 0.005,
                yield_pressure: 2.40,
            },
            MineralClass::BismuthCrystal => Self {
                class,
                rho: 1.20,
                bulk_modulus: 1.45,
                damping: 0.004,
                yield_pressure: 1.85,
            },
            MineralClass::AlienResonator => Self {
                class,
                rho: 1.10,
                bulk_modulus: 1.60,
                damping: 0.003,
                yield_pressure: 1.50,
            },
        }
    }

    #[inline]
    pub fn impedance(&self) -> f32 {
        (self.rho * self.bulk_modulus).sqrt()
    }
}

/// A free-rotating, destructible solid-state body coupled to the Dzog lattice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohesiveBody {
    pub id: u32,
    /// Center of mass position in world lattice coordinates.
    pub x: f32,
    pub y: f32,
    /// Center of mass linear velocity in cells/substep.
    pub vx: f32,
    pub vy: f32,
    /// Orientation angle in radians.
    pub theta: f32,
    /// Angular velocity in radians/substep.
    pub omega: f32,

    /// Dimension N of the square local grid (N x N).
    pub local_size: usize,
    /// Cell occupancy mask in the local grid (true if part of the original crystal hull).
    pub occupied: Vec<bool>,
    /// Continuous local cell cohesion C(lx, ly) in [0.0, 1.0].
    /// - C >= 0.55: intact solid boundary
    /// - 0.15 <= C < 0.55: semi-permeable cracked waveguide
    /// - C < 0.15: cavitated / severed void
    pub cohesion: Vec<f32>,

    pub material: BodyMaterialSpec,
    /// Total mass M = sum(rho * cell_area).
    pub mass: f32,
    /// Rotational moment of inertia I_z = sum(m_i * r_i^2) about center of mass.
    pub inertia: f32,
    /// Bounding radius from center of mass in grid cells.
    pub radius: f32,
    /// Last solver step when fractured (prevents same-frame double-hit artifacts).
    pub last_fracture_step: Option<usize>,
}

impl CohesiveBody {
    /// Generate a faceted polygonal crystalline asteroid on a local grid of size `local_size`.
    pub fn new_asteroid(
        id: u32,
        x: f32,
        y: f32,
        target_radius: f32,
        sides: usize,
        seed: u32,
        class: MineralClass,
    ) -> Self {
        let local_size = ((target_radius * 2.4).ceil() as usize).clamp(8, 28);
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;
        let sides = sides.clamp(5, 10);

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = lx as f32 - center;
                let dy = ly as f32 - center;
                let r = (dx * dx + dy * dy).sqrt();
                let angle = dy.atan2(dx);

                // Faceted crystalline polygon envelope with deterministic seed modulation
                let sector = ((angle + std::f32::consts::PI) / TAU) * (sides as f32);
                let sector_idx = sector.floor() as u32;
                let frac = sector.fract();
                let h0 = (((sector_idx.wrapping_mul(1103515245).wrapping_add(seed)) >> 16) & 0xFF) as f32 / 255.0;
                let next_sector = (sector_idx + 1) % (sides as u32);
                let h1 = (((next_sector.wrapping_mul(1103515245).wrapping_add(seed)) >> 16) & 0xFF) as f32 / 255.0;
                // Secondary angular harmonic for crisp mineralogical facets
                let sub_chord = (sector * std::f32::consts::PI).sin().abs() * 0.06;
                let facet_r = target_radius * (0.78 + 0.24 * (h0 * (1.0 - frac) + h1 * frac) - sub_chord);

                if r <= facet_r {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    // Natural internal crystal grain variation
                    cohesion[idx] = if r > facet_r - 0.8 { 0.92 } else { 1.0 };
                }
            }
        }

        let material = BodyMaterialSpec::for_class(class);
        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.0,
            omega: 0.0,
            local_size,
            occupied,
            cohesion,
            material,
            mass: 1.0,
            inertia: 1.0,
            radius: target_radius,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Generate an alien saucer-corvette hull with swept lateral wing pods and a resonant central core.
    pub fn new_saucer(id: u32, x: f32, y: f32, outer_radius: f32) -> Self {
        let local_size = ((outer_radius * 2.4).ceil() as usize).clamp(10, 22);
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = lx as f32 - center;
                let dy = (ly as f32 - center) * 1.28; // Saucer corvette aspect ratio
                let r = (dx * dx + dy * dy).sqrt();
                let wing_pod = dx.abs() <= outer_radius * 1.05 && (ly as f32 - center).abs() <= outer_radius * 0.34;
                if r <= outer_radius || wing_pod {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    // Central resonant core is slightly more sensitive to focused phase-lance shots
                    cohesion[idx] = if r < outer_radius * 0.35 { 0.78 } else { 0.98 };
                }
            }
        }

        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.0,
            omega: 0.0,
            local_size,
            occupied,
            cohesion,
            material: BodyMaterialSpec::for_class(MineralClass::AlienResonator),
            mass: 1.0,
            inertia: 1.0,
            radius: outer_radius,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Generate a twin-prong Harmonic Weaver cruiser hull with forward tuning-fork mandibles.
    pub fn new_weaver_ship(id: u32, x: f32, y: f32, outer_radius: f32) -> Self {
        let local_size = ((outer_radius * 2.4).ceil() as usize).clamp(10, 22);
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = lx as f32 - center;
                let dy = ly as f32 - center;
                let r = (dx * dx + dy * dy).sqrt();
                // Aft reactor core + twin forward resonance prongs along +X
                let in_core = r <= outer_radius * 0.85 && !(dx > outer_radius * 0.20 && dy.abs() < outer_radius * 0.28);
                let in_prongs = dx >= -outer_radius * 0.3 && dx <= outer_radius * 1.05
                    && (dy.abs() >= outer_radius * 0.25 && dy.abs() <= outer_radius * 0.68);
                if in_core || in_prongs {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    cohesion[idx] = if r < outer_radius * 0.35 { 0.80 } else { 0.96 };
                }
            }
        }

        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.0,
            omega: 0.0,
            local_size,
            occupied,
            cohesion,
            material: BodyMaterialSpec::for_class(MineralClass::AlienResonator),
            mass: 1.0,
            inertia: 1.0,
            radius: outer_radius,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Generate a Tri-Lobe Crystalline Interceptor hull (Port Wing, Central Core, Starboard Wing).
    ///
    /// Replaces discrete hitpoint counters (`hp: 3`) with a situated 3-lobe crystal structure:
    /// - Central Core (`|lx - center| <= core_half_w`)
    /// - Port Wing (`lx < center - core_half_w`)
    /// - Starboard Wing (`lx > center + core_half_w`)
    pub fn new_trilobe_ship(id: u32, x: f32, y: f32, outer_radius: f32) -> Self {
        let local_size = ((outer_radius * 2.4).ceil() as usize).clamp(13, 21);
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = lx as f32 - center;
                let dy = ly as f32 - center;
                // Nose points toward screen-top (-Y); aft thrusters sit toward screen-bottom (+Y)
                let up = -dy;
                let in_core = dx.abs() <= outer_radius * 0.34
                    && up >= -outer_radius * 0.72
                    && up <= outer_radius * 0.95
                    && (dx.abs() * 1.4 + up.max(0.0) * 0.55) <= outer_radius * 0.88;
                // Swept port and starboard resonator wings connected by narrow crystal necks
                let wing_up_center = -outer_radius * 0.10 - (dx.abs() - outer_radius * 0.34).max(0.0) * 0.25;
                let in_wing = dx.abs() > outer_radius * 0.34
                    && dx.abs() <= outer_radius * 1.02
                    && (up - wing_up_center).abs() <= outer_radius * 0.42;

                if in_core || in_wing {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    // Structural neck joints between wings and fuselage have slightly lower yield
                    // so wing hits cleanly sever the wing lobe before cavitating the core.
                    let is_neck = (dx.abs() - outer_radius * 0.38).abs() <= 0.75;
                    cohesion[idx] = if is_neck { 0.86 } else { 1.0 };
                }
            }
        }

        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.0,
            omega: 0.0,
            local_size,
            occupied,
            cohesion,
            material: BodyMaterialSpec::for_class(MineralClass::PiezoQuartz),
            mass: 1.0,
            inertia: 1.0,
            radius: outer_radius,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Generate a compact Piezo-Quartz annealing condensate ("Power Cube").
    pub fn new_power_cube(id: u32, x: f32, y: f32) -> Self {
        let local_size = 6;
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = (lx as f32 - center).abs();
                let dy = (ly as f32 - center).abs();
                if dx <= 1.8 && dy <= 1.8 {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    cohesion[idx] = 1.0;
                }
            }
        }

        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.35,
            omega: 0.04,
            local_size,
            occupied,
            cohesion,
            material: BodyMaterialSpec::for_class(MineralClass::PiezoQuartz),
            mass: 1.0,
            inertia: 1.0,
            radius: 2.2,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Generate a heavy twin-nacelle strike gunship hull (central bridge + port/starboard engine pods).
    ///
    /// Designed for multi-stage progressive fracturing: concentrated fire on a lateral nacelle or
    /// wing strut cleaves that pod off via BFS while the surviving core continues flying.
    pub fn new_twin_gunship(id: u32, x: f32, y: f32, outer_radius: f32) -> Self {
        let local_size = ((outer_radius * 2.4).ceil() as usize).clamp(12, 24);
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = lx as f32 - center;
                let dy = ly as f32 - center;
                // Forward axis is +X in local space (when theta = FRAC_PI_2, +X points downward +Y)
                let in_fuselage = dx >= -outer_radius * 0.78
                    && dx <= outer_radius * 0.92
                    && dy.abs() <= outer_radius * 0.26;
                let in_strut = dx >= -outer_radius * 0.32
                    && dx <= outer_radius * 0.36
                    && dy.abs() > outer_radius * 0.26
                    && dy.abs() <= outer_radius * 0.46;
                let in_nacelle = dx >= -outer_radius * 0.82
                    && dx <= outer_radius * 0.78
                    && dy.abs() > outer_radius * 0.46
                    && dy.abs() <= outer_radius * 0.94;

                if in_fuselage || in_strut || in_nacelle {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    cohesion[idx] = if in_strut { 0.84 } else { 0.98 };
                }
            }
        }

        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.0,
            omega: 0.0,
            local_size,
            occupied,
            cohesion,
            material: BodyMaterialSpec::for_class(MineralClass::BismuthCrystal),
            mass: 1.0,
            inertia: 1.0,
            radius: outer_radius,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Generate a faceted geodesic ground dome or biodome reactor structure.
    pub fn new_geodesic_dome(
        id: u32,
        x: f32,
        y: f32,
        radius: f32,
        class: MineralClass,
    ) -> Self {
        let local_size = ((radius * 2.3).ceil() as usize).clamp(10, 24);
        let count = local_size * local_size;
        let mut occupied = vec![false; count];
        let mut cohesion = vec![0.0f32; count];
        let center = (local_size as f32 - 1.0) * 0.5;

        for ly in 0..local_size {
            for lx in 0..local_size {
                let dx = lx as f32 - center;
                let dy = ly as f32 - center;
                let r = (dx * dx + dy * dy).sqrt();
                // Octagonal / geodesic outer envelope
                let oct_r = (dx.abs().max(dy.abs()) * 0.72 + r * 0.32).max(0.01);
                if r <= radius && oct_r <= radius * 0.98 {
                    let idx = ly * local_size + lx;
                    occupied[idx] = true;
                    let on_rib = (dx.abs() - dy.abs()).abs() < 0.75
                        || dx.abs() < 0.65
                        || dy.abs() < 0.65;
                    cohesion[idx] = if on_rib { 0.90 } else { 0.98 };
                }
            }
        }

        let mut body = Self {
            id,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            theta: 0.0,
            omega: 0.0,
            local_size,
            occupied,
            cohesion,
            material: BodyMaterialSpec::for_class(class),
            mass: 1.0,
            inertia: 1.0,
            radius,
            last_fracture_step: None,
        };
        body.recompute_mass_properties();
        body
    }

    /// Inspect structural integrity of a Tri-Lobe hull: `(port_wing_intact, core_intact, starboard_wing_intact)`.
    pub fn trilobe_status(&self) -> (bool, bool, bool) {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let neck_x = self.radius * 0.34;
        let mut port_active = 0usize;
        let mut port_total = 0usize;
        let mut core_active = 0usize;
        let mut core_total = 0usize;
        let mut star_active = 0usize;
        let mut star_total = 0usize;

        for ly in 0..self.local_size {
            for lx in 0..self.local_size {
                let idx = ly * self.local_size + lx;
                if !self.occupied[idx] {
                    continue;
                }
                let dx = lx as f32 - center;
                let active = self.cohesion[idx] >= 0.35;
                if dx < -neck_x {
                    port_total += 1;
                    if active {
                        port_active += 1;
                    }
                } else if dx > neck_x {
                    star_total += 1;
                    if active {
                        star_active += 1;
                    }
                } else {
                    core_total += 1;
                    if active {
                        core_active += 1;
                    }
                }
            }
        }

        let port_ok = port_total > 0 && port_active * 2 >= port_total;
        let core_ok = core_total > 0 && core_active * 3 >= core_total;
        let star_ok = star_total > 0 && star_active * 2 >= star_total;
        (port_ok, core_ok, star_ok)
    }

    /// Count intact structural lobes in `[0, 3]` for a Tri-Lobe hull.
    pub fn intact_lobe_count(&self) -> u32 {
        let (p, c, s) = self.trilobe_status();
        if !c {
            0
        } else {
            1 + u32::from(p) + u32::from(s)
        }
    }

    /// Sever a single structural lobe (`-1` = Port Wing, `1` = Starboard Wing, `0` = Central Core)
    /// via acoustic overpressure cavitation, returning the world coordinates of the severed lobe centroid.
    pub fn sever_trilobe_section(&mut self, lobe_sign: i32) -> (f32, f32) {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let neck_x = self.radius * 0.32;
        let mut sum_lx = 0.0f32;
        let mut sum_ly = 0.0f32;
        let mut cnt = 0usize;

        for ly in 0..self.local_size {
            for lx in 0..self.local_size {
                let idx = ly * self.local_size + lx;
                if !self.occupied[idx] {
                    continue;
                }
                let dx = lx as f32 - center;
                let match_lobe = match lobe_sign.cmp(&0) {
                    std::cmp::Ordering::Less => dx < -neck_x,
                    std::cmp::Ordering::Greater => dx > neck_x,
                    std::cmp::Ordering::Equal => dx.abs() <= neck_x,
                };
                if match_lobe && self.cohesion[idx] > 0.0 {
                    self.cohesion[idx] = 0.0;
                    sum_lx += lx as f32;
                    sum_ly += ly as f32;
                    cnt += 1;
                }
            }
        }

        self.recompute_mass_properties();
        if cnt > 0 {
            self.local_to_world(sum_lx / cnt as f32, sum_ly / cnt as f32)
        } else {
            (self.x, self.y)
        }
    }

    /// Exciton crystal annealing (reversible phase transition: `Delta C > 0`).
    ///
    /// Absorbing an annealing piezo-condensate ("Power Cube") restores eroded cohesion `C(lx, ly)`
    /// from the core outward, sealing internal fracture waveguides and regrowing severed wing lobes.
    /// Returns the number of cells restored or regrown.
    pub fn anneal_cells(&mut self, cohesion_budget: f32) -> usize {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let mut candidates: Vec<(usize, f32)> = Vec::new();

        for ly in 0..self.local_size {
            for lx in 0..self.local_size {
                let idx = ly * self.local_size + lx;
                if self.occupied[idx] && self.cohesion[idx] < 0.99 {
                    let dx = lx as f32 - center;
                    let dy = ly as f32 - center;
                    let dist = (dx * dx + dy * dy).sqrt();
                    candidates.push((idx, dist));
                }
            }
        }

        // Regrow from core outward so structural necks bond before outer wingtips
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut remaining = cohesion_budget.max(0.0);
        let mut healed_cells = 0usize;
        for (idx, _) in candidates {
            if remaining <= 0.001 {
                break;
            }
            let deficit = (1.0 - self.cohesion[idx]).max(0.0);
            let add = deficit.min(remaining);
            if add > 0.0 {
                self.cohesion[idx] = (self.cohesion[idx] + add).min(1.0);
                remaining -= add;
                healed_cells += 1;
            }
        }

        self.recompute_mass_properties();
        healed_cells
    }

    /// Number of active cells with cohesion >= 0.15.
    pub fn active_cell_count(&self) -> usize {
        self.occupied
            .iter()
            .zip(self.cohesion.iter())
            .filter(|(&occ, &c)| occ && c >= 0.15)
            .count()
    }

    /// Mean structural cohesion across occupied cells in `[0.0, 1.0]`.
    pub fn mean_cohesion(&self) -> f32 {
        let mut sum = 0.0f32;
        let mut cnt = 0usize;
        for (&occ, &c) in self.occupied.iter().zip(self.cohesion.iter()) {
            if occ {
                sum += c.clamp(0.0, 1.0);
                cnt += 1;
            }
        }
        if cnt == 0 { 0.0 } else { sum / cnt as f32 }
    }

    /// Fraction of originally occupied hull cells that remain structurally intact (`C >= 0.45`).
    pub fn structural_integrity(&self) -> f32 {
        let mut intact = 0usize;
        let mut total = 0usize;
        for (&occ, &c) in self.occupied.iter().zip(self.cohesion.iter()) {
            if occ {
                total += 1;
                if c >= 0.45 {
                    intact += 1;
                }
            }
        }
        if total == 0 {
            0.0
        } else {
            intact as f32 / total as f32
        }
    }

    /// Mean structural cohesion within the central reactor/cockpit core (`r <= 0.38 * local_size * 0.5`).
    pub fn core_cohesion(&self) -> f32 {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let core_r2 = (self.local_size as f32 * 0.20).max(1.8).powi(2);
        let mut sum = 0.0f32;
        let mut cnt = 0usize;
        for ly in 0..self.local_size {
            for lx in 0..self.local_size {
                let idx = ly * self.local_size + lx;
                if self.occupied[idx] {
                    let d2 = (lx as f32 - center).powi(2) + (ly as f32 - center).powi(2);
                    if d2 <= core_r2 {
                        sum += self.cohesion[idx].clamp(0.0, 1.0);
                        cnt += 1;
                    }
                }
            }
        }
        if cnt == 0 {
            self.mean_cohesion()
        } else {
            sum / cnt as f32
        }
    }

    /// Size (in cells) of the largest 4-connected structurally load-bearing component (`C >= 0.25`),
    /// along with the total number of originally occupied cells.
    pub fn largest_intact_component_ratio(&self) -> f32 {
        let n = self.local_size;
        let count = n * n;
        let total_occ = self.occupied.iter().filter(|&&o| o).count();
        if total_occ == 0 {
            return 0.0;
        }
        let mut visited = vec![false; count];
        let mut max_comp = 0usize;
        let mut queue = VecDeque::new();

        for sy in 0..n {
            for sx in 0..n {
                let s_idx = sy * n + sx;
                if visited[s_idx] || !self.occupied[s_idx] || self.cohesion[s_idx] < 0.25 {
                    continue;
                }
                let mut comp_len = 0usize;
                visited[s_idx] = true;
                queue.push_back((sx, sy));
                while let Some((cx, cy)) = queue.pop_front() {
                    comp_len += 1;
                    let neighbors = [
                        (cx.wrapping_sub(1), cy),
                        (cx + 1, cy),
                        (cx, cy.wrapping_sub(1)),
                        (cx, cy + 1),
                    ];
                    for (nx, ny) in neighbors {
                        if nx < n && ny < n {
                            let n_idx = ny * n + nx;
                            if !visited[n_idx] && self.occupied[n_idx] && self.cohesion[n_idx] >= 0.25 {
                                visited[n_idx] = true;
                                queue.push_back((nx, ny));
                            }
                        }
                    }
                }
                if comp_len > max_comp {
                    max_comp = comp_len;
                }
            }
        }
        max_comp as f32 / total_occ as f32
    }

    /// Recompute mass `M`, rotational moment of inertia `I_z`, and bounding `radius` from active cells.
    pub fn recompute_mass_properties(&mut self) {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let mut total_mass = 0.0f32;
        let mut total_inertia = 0.0f32;
        let mut max_r_sq = 1.0f32;

        for ly in 0..self.local_size {
            for lx in 0..self.local_size {
                let idx = ly * self.local_size + lx;
                if self.occupied[idx] && self.cohesion[idx] >= 0.15 {
                    let rx = lx as f32 - center;
                    let ry = ly as f32 - center;
                    let r_sq = rx * rx + ry * ry;
                    let cell_m = self.material.rho * (0.5 + 0.5 * self.cohesion[idx]);
                    total_mass += cell_m;
                    total_inertia += cell_m * (r_sq + 0.5);
                    if r_sq > max_r_sq {
                        max_r_sq = r_sq;
                    }
                }
            }
        }

        self.mass = total_mass.max(0.5);
        self.inertia = total_inertia.max(1.0);
        self.radius = max_r_sq.sqrt() + 0.75;
    }

    /// Map a world point `(wx, wy)` into local grid coordinates `(lx, ly)`, accounting for periodic wrapping.
    #[inline]
    pub fn world_to_local(&self, wx: f32, wy: f32, periodic_dims: Option<(f32, f32)>) -> (f32, f32) {
        let mut dx = wx - self.x;
        let mut dy = wy - self.y;
        if let Some((w, h)) = periodic_dims {
            if dx > 0.5 * w { dx -= w; } else if dx < -0.5 * w { dx += w; }
            if dy > 0.5 * h { dy -= h; } else if dy < -0.5 * h { dy += h; }
        }
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        // Rotate by -theta
        let rx = dx * cos_t + dy * sin_t;
        let ry = -dx * sin_t + dy * cos_t;
        let center = (self.local_size as f32 - 1.0) * 0.5;
        (rx + center, ry + center)
    }

    /// Map local grid coordinates `(lx, ly)` to world coordinates `(wx, wy)`.
    #[inline]
    pub fn local_to_world(&self, lx: f32, ly: f32) -> (f32, f32) {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let rx = lx - center;
        let ry = ly - center;
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        let wx = self.x + rx * cos_t - ry * sin_t;
        let wy = self.y + rx * sin_t + ry * cos_t;
        (wx, wy)
    }

    /// Bilinearly sample cohesion `C` in local grid coordinates `(lx, ly)`.
    #[inline]
    pub fn sample_local_cohesion(&self, lx: f32, ly: f32) -> f32 {
        let n = self.local_size as isize;
        let x0 = lx.floor() as isize;
        let y0 = ly.floor() as isize;
        if x0 < -1 || x0 >= n || y0 < -1 || y0 >= n {
            return 0.0;
        }
        let fx = lx - x0 as f32;
        let fy = ly - y0 as f32;

        let get_c = |ix: isize, iy: isize| -> f32 {
            if ix < 0 || ix >= n || iy < 0 || iy >= n {
                0.0
            } else {
                let idx = (iy as usize) * self.local_size + (ix as usize);
                if self.occupied[idx] { self.cohesion[idx] } else { 0.0 }
            }
        };

        let c00 = get_c(x0, y0);
        let c10 = get_c(x0 + 1, y0);
        let c01 = get_c(x0, y0 + 1);
        let c11 = get_c(x0 + 1, y0 + 1);

        let c0 = c00 * (1.0 - fx) + c10 * fx;
        let c1 = c01 * (1.0 - fx) + c11 * fx;
        c0 * (1.0 - fy) + c1 * fy
    }

    /// Evaluate sub-cell crystalline facet elevation, world-space surface normal `(nx, ny)`,
    /// and mineralogical ridge highlight at body-local coordinates `(lx, ly)`.
    #[inline]
    pub fn sample_crystal_facet(&self, lx: f32, ly: f32) -> (f32, f32, f32, f32) {
        let center = (self.local_size as f32 - 1.0) * 0.5;
        let dx = lx - center;
        let dy = ly - center;
        let r = (dx * dx + dy * dy).sqrt().max(0.001);
        let r_norm = (r / self.radius.max(1.0)).clamp(0.0, 1.2);
        let angle = dy.atan2(dx);

        // Local cohesion gradient for bevels & internal fracture walls
        let coh_e = self.sample_local_cohesion(lx + 0.65, ly);
        let coh_w = self.sample_local_cohesion(lx - 0.65, ly);
        let coh_n = self.sample_local_cohesion(lx, ly + 0.65);
        let coh_s = self.sample_local_cohesion(lx, ly - 0.65);
        let bevel_nx = -(coh_e - coh_w);
        let bevel_ny = -(coh_n - coh_s);

        let (local_nx, local_ny, ridge) = match self.material.class {
            MineralClass::PiezoQuartz => {
                // 6-fold hexagonal crystal prism facets
                let sector = ((angle + std::f32::consts::PI) / (TAU / 6.0)).floor();
                let facet_ang = sector * (TAU / 6.0) + (TAU / 12.0) - std::f32::consts::PI;
                let edge_prox = ((angle * 3.0).sin().abs() < 0.16) as i32 as f32;
                let inner_ring = ((r_norm - 0.52).abs() < 0.08) as i32 as f32;
                (
                    facet_ang.cos() * 0.65 + bevel_nx * 0.6,
                    facet_ang.sin() * 0.65 + bevel_ny * 0.6,
                    (edge_prox * 0.55 + inner_ring * 0.45).clamp(0.0, 1.0),
                )
            }
            MineralClass::ObsidianAnvil => {
                // Sharp angular conchoidal fracture planes & dark volcanic ridges
                let seed_ang = (self.id as f32) * 0.73;
                let sector = ((angle + std::f32::consts::PI + seed_ang) / (TAU / 5.0)).floor();
                let facet_ang = sector * (TAU / 5.0) - std::f32::consts::PI;
                let ridge_line = ((dx * 0.65 + dy * 0.45).sin().abs() < 0.18
                    || (r_norm > 0.76)) as i32 as f32;
                (
                    facet_ang.cos() * 0.75 + bevel_nx * 0.7,
                    facet_ang.sin() * 0.75 + bevel_ny * 0.7,
                    ridge_line * 0.75,
                )
            }
            MineralClass::BismuthCrystal => {
                // Iconic stepped hopper-crystal rectangular terraces
                let box_r = dx.abs().max(dy.abs()) / self.radius.max(1.0);
                let terrace = (box_r * 4.5).fract();
                let step_rim = if terrace < 0.22 || terrace > 0.82 { 0.85 } else { 0.15 };
                let sx = if dx.abs() >= dy.abs() { dx.signum() } else { 0.0 };
                let sy = if dy.abs() > dx.abs() { dy.signum() } else { 0.0 };
                (
                    sx * 0.65 + bevel_nx * 0.55,
                    sy * 0.65 + bevel_ny * 0.55,
                    step_rim,
                )
            }
            MineralClass::AlienResonator => {
                let ring = ((r_norm - 0.55).abs() < 0.12) as i32 as f32;
                (
                    (dx / r) * 0.6 + bevel_nx * 0.5,
                    (dy / r) * 0.6 + bevel_ny * 0.5,
                    ring * 0.7,
                )
            }
        };

        // Rotate local normal into world frame (+theta)
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        let w_nx = local_nx * cos_t - local_ny * sin_t;
        let w_ny = local_nx * sin_t + local_ny * cos_t;
        let elev = (1.0 - r_norm * 0.55).clamp(0.15, 1.0);
        (elev, w_nx, w_ny, ridge)
    }

    /// Rasterize this rotating body into the world `MaterialField` and `BoundaryMask`.
    ///
    /// - Intact cells (`C >= 0.55`) block staggered velocity faces and impose local rigid + rotational
    ///   velocity `v_wall = v_cm + omega x r`.
    /// - Cracked waveguide cells (`0.15 <= C < 0.55`) unblock boundary faces so acoustic waves
    ///   propagate through internal cleavage fissures while preserving solid material impedance.
    pub fn rasterize_to_lattice(
        &self,
        material: &mut MaterialField,
        boundary: &mut BoundaryMask,
        periodic: bool,
    ) {
        let w = material.config.width as isize;
        let h = material.config.height as isize;
        let r_bound = (self.radius.ceil() as isize) + 1;
        let cx = self.x.round() as isize;
        let cy = self.y.round() as isize;
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        let center = (self.local_size as f32 - 1.0) * 0.5;

        for oy in -r_bound..=r_bound {
            for ox in -r_bound..=r_bound {
                let px = cx + ox;
                let py = cy + oy;
                if !periodic && (px < 1 || px >= w - 1 || py < 1 || py >= h - 1) {
                    continue;
                }
                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;

                let dx = px as f32 - self.x;
                let dy = py as f32 - self.y;
                let lx = dx * cos_t + dy * sin_t + center;
                let ly = -dx * sin_t + dy * cos_t + center;
                let coh = self.sample_local_cohesion(lx, ly);
                if coh < 0.15 {
                    continue;
                }

                let idx = material.config.index(gx, gy);
                material.density[idx] = self.material.rho;
                material.bulk_modulus[idx] = self.material.bulk_modulus;
                material.damping[idx] = if coh < 0.55 {
                    self.material.damping.max(0.03)
                } else {
                    self.material.damping
                };
                material.cohesion[idx] = coh;
                material.yield_pressure[idx] = self.material.yield_pressure;

                if coh >= 0.55 {
                    // In clean quantum vacuum, solid hulls act as reflective boundaries without
                    // continuously pumping fluid piston wakes ("water sloshing") during free drift.
                    boundary.wall_east[idx] = true;
                    boundary.wall_vx[idx] = 0.0;
                    boundary.wall_north[idx] = true;
                    boundary.wall_vy[idx] = 0.0;
                }
            }
        }
    }

    /// Integrate acoustic radiation pressure `(-grad p)`, Bohmian quantum potential guidance `(-grad Q)`,
    /// and overpressure cohesion erosion over the body's rotating contour.
    ///
    /// Returns `(force_x, force_y, torque_z)` and updates `self.vx`, `self.vy`, `self.omega`.
    pub fn integrate_field_forces(
        &mut self,
        state: &AcousticState,
        dt_sub: f32,
        periodic: bool,
    ) -> (f32, f32, f32) {
        let w = state.config.width as isize;
        let h = state.config.height as isize;
        let r_bound = (self.radius.ceil() as isize) + 1;
        let cx = self.x.round() as isize;
        let cy = self.y.round() as isize;
        let periodic_dims = if periodic { Some((w as f32, h as f32)) } else { None };

        let mut fx_total = 0.0f32;
        let mut fy_total = 0.0f32;
        let mut tau_total = 0.0f32;

        for oy in -r_bound..=r_bound {
            for ox in -r_bound..=r_bound {
                let px = cx + ox;
                let py = cy + oy;
                if !periodic && (px < 2 || px >= w - 2 || py < 2 || py >= h - 2) {
                    continue;
                }
                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;

                let (lx, ly) = self.world_to_local(px as f32, py as f32, periodic_dims);
                let ix = lx.round() as isize;
                let iy = ly.round() as isize;
                if ix < 0 || ix >= self.local_size as isize || iy < 0 || iy >= self.local_size as isize {
                    continue;
                }
                let l_idx = (iy as usize) * self.local_size + (ix as usize);
                if !self.occupied[l_idx] || self.cohesion[l_idx] < 0.15 {
                    continue;
                }

                // Sample local pressure gradient and Bohmian quantum force
                let (grad_px, grad_py) = state.pressure_gradient_wrapped_at(gx, gy, periodic);
                let grad_mag_sq = grad_px * grad_px + grad_py * grad_py;
                let p_abs = state.pressure[state.config.index(gx, gy)].abs();
                if grad_mag_sq > 0.04 {
                    let (q_fx, q_fy) = state.quantum_force_wrapped_at(gx, gy, periodic);
                    let dfx = -grad_px * 0.18 + q_fx * 0.025;
                    let dfy = -grad_py * 0.18 + q_fy * 0.025;

                    let rx = px as f32 - self.x;
                    let ry = py as f32 - self.y;
                    fx_total += dfx;
                    fy_total += dfy;
                    tau_total += rx * dfy - ry * dfx;
                } else if self.radius <= 3.2 && p_abs > 0.015 {
                    let (q_fx, q_fy) = state.quantum_force_wrapped_at(gx, gy, periodic);
                    let dfx = q_fx * 0.045;
                    let dfy = q_fy * 0.045;

                    let rx = px as f32 - self.x;
                    let ry = py as f32 - self.y;
                    fx_total += dfx;
                    fy_total += dfy;
                    tau_total += rx * dfy - ry * dfx;
                }

                // Continuous cohesion erosion when local acoustic pressure exceeds yield threshold
                if p_abs > self.material.yield_pressure {
                    let excess = p_abs - self.material.yield_pressure;
                    self.cohesion[l_idx] = (self.cohesion[l_idx] - 0.18 * excess * dt_sub).max(0.0);
                }
            }
        }

        self.vx = (self.vx + (fx_total / self.mass) * dt_sub).clamp(-0.65, 0.65);
        self.vy = (self.vy + (fy_total / self.mass) * dt_sub).clamp(-0.65, 0.65);
        self.omega = (self.omega + (tau_total / self.inertia) * dt_sub).clamp(-0.14, 0.14);

        (fx_total, fy_total, tau_total)
    }

    /// Apply an external impulse `(jx, jy)` at world contact point `(wx, wy)`, imparting both
    /// linear momentum `dP = J` and angular torque impulse `dL_z = (r x J)_z`.
    pub fn apply_contact_impulse(
        &mut self,
        wx: f32,
        wy: f32,
        jx: f32,
        jy: f32,
        periodic_dims: Option<(f32, f32)>,
    ) {
        let mut rx = wx - self.x;
        let mut ry = wy - self.y;
        if let Some((w, h)) = periodic_dims {
            if rx > 0.5 * w { rx -= w; } else if rx < -0.5 * w { rx += w; }
            if ry > 0.5 * h { ry -= h; } else if ry < -0.5 * h { ry += h; }
        }

        self.vx = (self.vx + jx / self.mass).clamp(-0.65, 0.65);
        self.vy = (self.vy + jy / self.mass).clamp(-0.65, 0.65);
        let d_tau = rx * jy - ry * jx;
        self.omega = (self.omega + d_tau / self.inertia).clamp(-0.16, 0.16);
    }

    /// Carve directional crystalline cleavage planes through the body's local grid from world impact
    /// point `(wx, wy)` along propagation vector `(dir_x, dir_y)`.
    ///
    /// Returns `true` if this strike triggered a **Resonance Crit** (striking an already-cracked vein
    /// or using an overcharged bolt (`energy >= 1.8`) to cleave all the way through the crystal).
    pub fn carve_cleavage_fissure(
        &mut self,
        wx: f32,
        wy: f32,
        dir_x: f32,
        dir_y: f32,
        energy: f32,
        periodic_dims: Option<(f32, f32)>,
    ) -> bool {
        let (lx_raw, ly_raw) = self.world_to_local(wx, wy, periodic_dims);
        let n_f = (self.local_size.saturating_sub(1)) as f32;
        let lx = lx_raw.clamp(0.0, n_f);
        let ly = ly_raw.clamp(0.0, n_f);

        // Rotate propagation vector into body-local frame (-theta)
        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        let l_dx = dir_x * cos_t + dir_y * sin_t;
        let l_dy = -dir_x * sin_t + dir_y * cos_t;
        let base_angle = l_dy.atan2(l_dx);

        // Check if impact point is near an existing cracked cleavage vein (Resonance Crit!)
        let mut hit_existing_crack = false;
        for ny in 0..self.local_size {
            for nx in 0..self.local_size {
                let idx = ny * self.local_size + nx;
                if self.occupied[idx] && self.cohesion[idx] < 0.65 {
                    let d2 = (nx as f32 - lx).powi(2) + (ny as f32 - ly).powi(2);
                    if d2 <= 6.25 {
                        hit_existing_crack = true;
                        break;
                    }
                }
            }
        }

        let full_cleave = hit_existing_crack || energy >= 1.75 || self.mean_cohesion() < 0.78;

        // 1. Carve impact spall crater around (lx, ly)
        let crater_r = if full_cleave { 2.4 } else { 1.8 };
        for ny in 0..self.local_size {
            for nx in 0..self.local_size {
                let idx = ny * self.local_size + nx;
                if !self.occupied[idx] {
                    continue;
                }
                let dist = ((nx as f32 - lx).powi(2) + (ny as f32 - ly).powi(2)).sqrt();
                if dist <= crater_r {
                    let target_c = if full_cleave { 0.0 } else { 0.25 };
                    self.cohesion[idx] = self.cohesion[idx].min(target_c);
                }
            }
        }

        // 2. Trace primary cleavage ray + two shear branches
        let branches: &[(f32, f32)] = if full_cleave {
            // Through-going cleavage plane that severs the crystal into 2-3 topological components
            &[(0.0, 0.0), (std::f32::consts::PI, 0.0), (0.62, 0.08)]
        } else {
            // Partial waveguide fissure (0.28 cohesion) that channels phonons without severing yet
            &[(0.0, 0.28), (0.50, 0.35), (-0.50, 0.35)]
        };

        let max_steps = self.local_size * 2;
        for &(ang_off, target_coh) in branches {
            let ang = base_angle + ang_off;
            let vx = ang.cos();
            let vy = ang.sin();
            let perp_x = -vy;
            let perp_y = vx;

            for step in 0..max_steps {
                let t = step as f32 * 0.45;
                let px = lx + vx * t;
                let py = ly + vy * t;

                // Ensure 4-connected separation by carving a 1.5-cell wide seam when full_cleave is active
                let widths: &[f32] = if full_cleave { &[-0.55, 0.0, 0.55] } else { &[0.0] };
                for &w_off in widths {
                    let ix = (px + perp_x * w_off).round() as isize;
                    let iy = (py + perp_y * w_off).round() as isize;
                    if ix >= 0 && ix < self.local_size as isize && iy >= 0 && iy < self.local_size as isize {
                        let idx = (iy as usize) * self.local_size + (ix as usize);
                        if self.occupied[idx] {
                            self.cohesion[idx] = self.cohesion[idx].min(target_coh);
                        }
                    }
                }
            }
        }

        full_cleave
    }

    /// Progressive multi-stage fracture propagation for medium and large hulls & structures.
    ///
    /// - Initial strikes on intact armor carve localized glowing fissures (`C in 0.22..0.38`) without
    ///   instantly severing the hull.
    /// - Subsequent strikes hitting near existing fractures (`C < 0.68`) amplify fracture growth:
    ///   eroding existing cracks toward cavitation (`C -> 0.0`), extending fracture length, and
    ///   branching secondary cleavage seams until the section topologically cleaves via BFS.
    /// - Returns `(triggered_full_cleave, existing_fractured_cells_hit)`.
    pub fn inflict_progressive_fracture(
        &mut self,
        wx: f32,
        wy: f32,
        dir_x: f32,
        dir_y: f32,
        energy: f32,
        periodic_dims: Option<(f32, f32)>,
    ) -> (bool, usize) {
        let (lx_raw, ly_raw) = self.world_to_local(wx, wy, periodic_dims);
        let n_f = (self.local_size.saturating_sub(1)) as f32;
        let lx = lx_raw.clamp(0.0, n_f);
        let ly = ly_raw.clamp(0.0, n_f);

        let cos_t = self.theta.cos();
        let sin_t = self.theta.sin();
        let l_dx = dir_x * cos_t + dir_y * sin_t;
        let l_dy = -dir_x * sin_t + dir_y * cos_t;
        let base_angle = l_dy.atan2(l_dx);

        // Count existing fractured cells near the impact site and sympathetically deepen them
        let mut nearby_cracked = 0usize;
        for ny in 0..self.local_size {
            for nx in 0..self.local_size {
                let idx = ny * self.local_size + nx;
                if !self.occupied[idx] {
                    continue;
                }
                let d2 = (nx as f32 - lx).powi(2) + (ny as f32 - ly).powi(2);
                if d2 <= 12.25 && self.cohesion[idx] < 0.68 {
                    nearby_cracked += 1;
                    // Hitting an existing fracture erodes surrounding cracked cells further
                    let erode = (0.20 * energy * (1.0 - (d2.sqrt() / 3.8).min(0.85))).max(0.06);
                    self.cohesion[idx] = (self.cohesion[idx] - erode).max(0.0);
                }
            }
        }

        let mean_c = self.mean_cohesion();
        let stage = if energy >= 2.05 || nearby_cracked >= 4 || mean_c < 0.72 {
            3 // Full through-cleave stage
        } else if nearby_cracked >= 1 || mean_c < 0.86 {
            2 // Branched deep fracture stage
        } else {
            1 // Initial surface & waveguide crack stage
        };

        let crater_r = match stage {
            1 => 0.85,
            2 => 1.15,
            _ => 1.45,
        };
        let crater_coh = match stage {
            1 => 0.30,
            2 => 0.10,
            _ => 0.0,
        };
        for ny in 0..self.local_size {
            for nx in 0..self.local_size {
                let idx = ny * self.local_size + nx;
                if !self.occupied[idx] {
                    continue;
                }
                let dist = ((nx as f32 - lx).powi(2) + (ny as f32 - ly).powi(2)).sqrt();
                if dist <= crater_r {
                    self.cohesion[idx] = self.cohesion[idx].min(crater_coh);
                }
            }
        }

        let branches: &[(f32, f32, f32)] = match stage {
            1 => &[
                (0.0, 0.32, 0.52),
                (0.48, 0.38, 0.36),
                (-0.48, 0.38, 0.36),
            ],
            2 => &[
                (0.0, 0.08, 0.85),
                (std::f32::consts::PI, 0.08, 0.65),
                (0.55, 0.22, 0.60),
                (-0.55, 0.22, 0.60),
            ],
            _ => &[
                (0.0, 0.0, 1.40),
                (std::f32::consts::PI, 0.0, 1.40),
                (0.58, 0.0, 1.10),
                (-0.58, 0.08, 0.90),
            ],
        };

        let full_cleave = stage >= 3;
        for &(ang_off, target_coh, len_frac) in branches {
            let ang = base_angle + ang_off;
            let vx = ang.cos();
            let vy = ang.sin();
            let perp_x = -vy;
            let perp_y = vx;
            let max_steps = ((self.local_size as f32 * len_frac * 1.6) as usize).max(4);
            let widths: &[f32] = if full_cleave && target_coh == 0.0 {
                &[-0.55, 0.0, 0.55]
            } else if stage >= 2 {
                &[-0.35, 0.0, 0.35]
            } else {
                &[0.0]
            };

            for step in 0..max_steps {
                let t = step as f32 * 0.45;
                let px = lx + vx * t;
                let py = ly + vy * t;
                for &w_off in widths {
                    let ix = (px + perp_x * w_off).round() as isize;
                    let iy = (py + perp_y * w_off).round() as isize;
                    if ix >= 0
                        && ix < self.local_size as isize
                        && iy >= 0
                        && iy < self.local_size as isize
                    {
                        let idx = (iy as usize) * self.local_size + (ix as usize);
                        if self.occupied[idx] {
                            self.cohesion[idx] = self.cohesion[idx].min(target_coh);
                        }
                    }
                }
            }
        }

        self.recompute_mass_properties();
        (full_cleave, nearby_cracked)
    }

    /// Run 4-connected Breadth-First Search (BFS) over intact/waveguide cells (`C >= 0.15`).
    ///
    /// If the body's topology has been cleaved into `k >= 2` connected components (each >= 4 cells),
    /// splits the parent body into `k` independent child `CohesiveBody` fragments conserving
    /// linear and angular momentum plus outward cavitation separation velocity.
    pub fn split_connected_components(
        &self,
        next_id: &mut u32,
        periodic_dims: Option<(f32, f32)>,
    ) -> Vec<CohesiveBody> {
        let n = self.local_size;
        let count = n * n;
        let mut visited = vec![false; count];
        let mut components: Vec<Vec<(usize, usize, f32)>> = Vec::new();

        for sy in 0..n {
            for sx in 0..n {
                let s_idx = sy * n + sx;
                if visited[s_idx] || !self.occupied[s_idx] || self.cohesion[s_idx] < 0.15 {
                    continue;
                }

                let mut comp = Vec::new();
                let mut queue = VecDeque::new();
                visited[s_idx] = true;
                queue.push_back((sx, sy));

                while let Some((cx, cy)) = queue.pop_front() {
                    let c_idx = cy * n + cx;
                    comp.push((cx, cy, self.cohesion[c_idx]));

                    let neighbors = [
                        (cx.wrapping_sub(1), cy),
                        (cx + 1, cy),
                        (cx, cy.wrapping_sub(1)),
                        (cx, cy + 1),
                    ];
                    for (nx, ny) in neighbors {
                        if nx < n && ny < n {
                            let n_idx = ny * n + nx;
                            if !visited[n_idx] && self.occupied[n_idx] && self.cohesion[n_idx] >= 0.15 {
                                visited[n_idx] = true;
                                queue.push_back((nx, ny));
                            }
                        }
                    }
                }

                if comp.len() >= 4 {
                    components.push(comp);
                }
            }
        }

        if components.len() < 2 {
            return Vec::new();
        }

        let mut children = Vec::with_capacity(components.len());

        for comp in components {
            // Compute component centroid in parent local coordinates
            let mut sum_lx = 0.0f32;
            let mut sum_ly = 0.0f32;
            for &(lx, ly, _) in &comp {
                sum_lx += lx as f32;
                sum_ly += ly as f32;
            }
            let mean_lx = sum_lx / comp.len() as f32;
            let mean_ly = sum_ly / comp.len() as f32;

            // Map child centroid to world coordinates
            let (mut world_cx, mut world_cy) = self.local_to_world(mean_lx, mean_ly);
            if let Some((w, h)) = periodic_dims {
                world_cx = world_cx.rem_euclid(w);
                world_cy = world_cy.rem_euclid(h);
            }

            // Determine bounding radius of child component
            let mut max_r = 2.0f32;
            for &(lx, ly, _) in &comp {
                let d = ((lx as f32 - mean_lx).powi(2) + (ly as f32 - mean_ly).powi(2)).sqrt();
                if d > max_r {
                    max_r = d;
                }
            }
            let child_size = ((max_r * 2.4).ceil() as usize).clamp(6, 20);
            let child_center = (child_size as f32 - 1.0) * 0.5;
            let mut child_occ = vec![false; child_size * child_size];
            let mut child_coh = vec![0.0f32; child_size * child_size];

            for &(lx, ly, coh) in &comp {
                let clx = (lx as f32 - mean_lx + child_center).round() as isize;
                let cly = (ly as f32 - mean_ly + child_center).round() as isize;
                if clx >= 0 && clx < child_size as isize && cly >= 0 && cly < child_size as isize {
                    let c_idx = (cly as usize) * child_size + (clx as usize);
                    child_occ[c_idx] = true;
                    // Freshly cleaved crystal faces anneal to solid cohesion (0.88)
                    child_coh[c_idx] = coh.max(0.88);
                }
            }

            // Momentum partition: v_child = v_cm + omega x r_offset + cavitation separation impulse
            let (raw_wx, raw_wy) = self.local_to_world(mean_lx, mean_ly);
            let rx = raw_wx - self.x;
            let ry = raw_wy - self.y;
            let r_len = (rx * rx + ry * ry).sqrt().max(0.5);
            let sep_speed = 0.14;
            let child_vx = (self.vx - self.omega * ry + (rx / r_len) * sep_speed).clamp(-0.60, 0.60);
            let child_vy = (self.vy + self.omega * rx + (ry / r_len) * sep_speed).clamp(-0.60, 0.60);
            let child_omega = (self.omega + (rx * sep_speed - ry * sep_speed) * 0.04).clamp(-0.14, 0.14);

            let child_id = *next_id;
            *next_id += 1;

            let mut child = CohesiveBody {
                id: child_id,
                x: world_cx,
                y: world_cy,
                vx: child_vx,
                vy: child_vy,
                theta: self.theta,
                omega: child_omega,
                local_size: child_size,
                occupied: child_occ,
                cohesion: child_coh,
                material: self.material,
                mass: 1.0,
                inertia: 1.0,
                radius: max_r,
                last_fracture_step: self.last_fracture_step,
            };
            child.recompute_mass_properties();
            children.push(child);
        }

        children
    }
}
