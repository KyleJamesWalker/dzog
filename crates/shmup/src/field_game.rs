//! Core Simulation State & Macro-Step Loop for *Shmup* (`crates/shmup/src/field_game.rs`).
//!
//! Coordinate convention (matching Bevy's top-left texture UV mapping):
//! - `y = 0` is the **top** of the screen (where enemy squadrons, ground structures, and asteroids
//!   spawn offscreen at `y < 0` and scroll downward with `+vy > 0`).
//! - `y = height` is the **bottom** of the screen (where the player's Tri-Lobe Crystalline Ship
//!   is stationed at `y ~ height * 0.82`, pointing upward toward `-Y`).

use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::{Arc, Mutex};
pub use dzog_bevy::{FieldViewMode, GpuFieldAccelerator};
use dzog_core::*;
use dzog_wgpu::GpuBodyQuery;

use crate::canyon::{
    canyon_wall_bounds, sample_building_height_and_normal, stamp_canyon_into_lattice,
};
use crate::entities::{
    AlienRole, AlienVessel, GroundTarget, GroundTargetKind, GroundedWreckage, PowerCube,
    ProjectileKind, ScorchedCrater, ShmupBranchSnapshot, ShmupEvent, SolitonBolt,
};

pub struct ShmupFieldGame {
    pub solver: AcousticSolver,
    pub gpu_accel: GpuFieldAccelerator,
    pub width: usize,
    pub height: usize,

    /// Situated 3-lobe crystalline player ship hull (`intact_lobe_count() in 0..=3`).
    pub player_hull: CohesiveBody,
    pub invulnerable_steps: u16,
    pub shield_visual_timer: u8,
    pub thruster_visual_timer: u8,
    pub capacitor: f32,
    pub max_capacitor: f32,

    /// Sky and ground actors coupled to the lattice.
    pub asteroids: Vec<CohesiveBody>,
    pub grounded_wreckage: Vec<GroundedWreckage>,
    pub aliens: Vec<AlienVessel>,
    pub ground_targets: Vec<GroundTarget>,
    pub craters: Vec<ScorchedCrater>,
    pub power_cubes: Vec<PowerCube>,
    pub bolts: Vec<SolitonBolt>,
    pub next_body_id: u32,

    /// Downward (`+Y`) wave-stream scroll velocity and phase.
    pub scroll_speed: f32,
    pub canyon_scroll: f32,
    pub spawn_timer: u16,
    pub pipeline_flash_timer: u8,

    /// Binaural cockpit sonifier & shared lock-free audio telemetry frame.
    pub sonifier: BinauralCockpitSonifier,
    pub last_audio_frame: FieldAcousticFrame,
    pub shared_audio_frame: Arc<Mutex<FieldAcousticFrame>>,

    pub score: u32,
    pub credits: u32,
    pub high_score: u32,
    pub wave_number: u32,
    pub topological_cleaves: u32,
    pub chain_detonations: u32,
    pub ground_scars_count: u32,
    pub cubes_collected_player: u32,
    pub cubes_collected_aliens: u32,
    pub in_play: bool,

    pub view_mode: FieldViewMode,
    pub wave_force_enabled: bool,
    pub autopilot_enabled: bool,
    pub autopilot_tick: u32,

    pub saved_branch: Option<ShmupBranchSnapshot>,
    pub branch_divergence_l2: f32,
    pub field_energy: FieldEnergy,
    pub pixel_buffer: Vec<u8>,
}

impl ShmupFieldGame {
    /// Default 16:9 (1080p aspect ratio) physics lattice dimensions (`240 x 135`).
    pub const DEFAULT_WIDTH: usize = 240;
    pub const DEFAULT_HEIGHT: usize = 135;
    /// Sub-cell perceptual render supersampling factor (`4x` -> `960x540`, exact 1/2 of 1080p and 1/4 of 4K).
    pub const RENDER_SCALE: usize = 4;

    #[inline]
    pub fn render_width(&self) -> usize {
        self.width * Self::RENDER_SCALE
    }

    #[inline]
    pub fn render_height(&self) -> usize {
        self.height * Self::RENDER_SCALE
    }

    pub fn new(width: usize, height: usize) -> Self {
        let grid = GridConfig::new(width, height, 1.0, 1.0);
        let state = AcousticState::new(grid);
        let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.018);
        material.density[0] = 0.70;
        material.bulk_modulus[0] = 2.10;
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        material.density[0] = 1.0;
        material.bulk_modulus[0] = 1.0;

        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::AbsorbingSponge);
        let nonlinearity = NonlinearLaw {
            beta: 0.08,
            alpha: 1.20,
            threshold_decay: 0.055,
        };
        let config = SolverConfig::new(dt, 0.7, 4).with_nonlinearity(nonlinearity);
        let gpu_accel = GpuFieldAccelerator::new(&state, &material, &boundary, &config);
        let solver = AcousticSolver::new(state, material, boundary, config)
            .expect("Failed to initialize Shmup AcousticSolver");

        let player_hull = CohesiveBody::new_trilobe_ship(
            1,
            width as f32 * 0.5,
            height as f32 * 0.82,
            6.5,
        );

        let mut game = Self {
            solver,
            gpu_accel,
            width,
            height,
            player_hull,
            invulnerable_steps: 30,
            shield_visual_timer: 0,
            thruster_visual_timer: 0,
            capacitor: 35.0,
            max_capacitor: 100.0,
            asteroids: Vec::with_capacity(32),
            grounded_wreckage: Vec::with_capacity(32),
            aliens: Vec::with_capacity(16),
            ground_targets: Vec::with_capacity(24),
            craters: Vec::with_capacity(32),
            power_cubes: Vec::with_capacity(32),
            bolts: Vec::with_capacity(64),
            next_body_id: 10,
            scroll_speed: 0.24,
            canyon_scroll: 0.0,
            spawn_timer: 0,
            pipeline_flash_timer: 0,
            sonifier: BinauralCockpitSonifier::new(3.5, 512),
            last_audio_frame: FieldAcousticFrame::default(),
            shared_audio_frame: Arc::new(Mutex::new(FieldAcousticFrame::default())),
            score: 0,
            credits: 0,
            high_score: 0,
            wave_number: 1,
            topological_cleaves: 0,
            chain_detonations: 0,
            ground_scars_count: 0,
            cubes_collected_player: 0,
            cubes_collected_aliens: 0,
            in_play: true,
            view_mode: FieldViewMode::CompositeArcade,
            wave_force_enabled: true,
            autopilot_enabled: false,
            autopilot_tick: 0,
            saved_branch: None,
            branch_divergence_l2: 0.0,
            field_energy: FieldEnergy {
                potential: 0.0,
                kinetic: 0.0,
                total: 0.0,
            },
            pixel_buffer: vec![
                0u8;
                width * height * Self::RENDER_SCALE * Self::RENDER_SCALE * 4
            ],
        };

        game.spawn_wave_formation(1);
        game.sync_all_bodies_to_lattice();
        game.field_energy = FieldEnergy::compute(&game.solver.state, &game.solver.material);
        game
    }

    /// Number of intact structural lobes on the player's Tri-Lobe hull (`0..=3`).
    #[inline]
    pub fn player_intact_lobes(&self) -> u32 {
        self.player_hull.intact_lobe_count()
    }

    /// 4-way planar reaction thruster control (`WASD` or `Arrow Keys`).
    pub fn thrust_planar(&mut self, dir_x: f32, dir_y: f32) {
        let mag = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if mag < 1e-3 {
            return;
        }
        let nx = dir_x / mag;
        let ny = dir_y / mag;

        let (port_ok, _, star_ok) = self.player_hull.trilobe_status();
        let wing_factor = 0.74 + 0.13 * (f32::from(port_ok) + f32::from(star_ok));
        let accel = 0.105 * wing_factor;

        self.player_hull.vx = (self.player_hull.vx + nx * accel).clamp(-1.05, 1.05);
        self.player_hull.vy = (self.player_hull.vy + ny * accel).clamp(-0.95, 0.90);
        self.player_hull.theta = (nx * 0.22).clamp(-0.28, 0.28);
        self.thruster_visual_timer = 5;
    }

    /// Save a counterfactual situation branch (`[B]`).
    pub fn save_branch_checkpoint(&mut self) {
        self.saved_branch = Some(ShmupBranchSnapshot {
            checkpoint: Checkpoint::capture(&self.solver),
            player_hull: self.player_hull.clone(),
            capacitor: self.capacitor,
            asteroids: self.asteroids.clone(),
            grounded_wreckage: self.grounded_wreckage.clone(),
            aliens: self.aliens.clone(),
            ground_targets: self.ground_targets.clone(),
            craters: self.craters.clone(),
            power_cubes: self.power_cubes.clone(),
            bolts: self.bolts.clone(),
            canyon_scroll: self.canyon_scroll,
            score: self.score,
            credits: self.credits,
            wave_number: self.wave_number,
        });
        self.branch_divergence_l2 = 0.0;
    }

    /// Restore the saved counterfactual branch (`[U]`).
    pub fn restore_branch_checkpoint(&mut self) -> bool {
        if let Some(snap) = &self.saved_branch {
            if let Ok(restored) = snap.checkpoint.restore() {
                self.solver = restored;
                self.player_hull = snap.player_hull.clone();
                self.capacitor = snap.capacitor;
                self.asteroids = snap.asteroids.clone();
                self.grounded_wreckage = snap.grounded_wreckage.clone();
                self.aliens = snap.aliens.clone();
                self.ground_targets = snap.ground_targets.clone();
                self.craters = snap.craters.clone();
                self.power_cubes = snap.power_cubes.clone();
                self.bolts = snap.bolts.clone();
                self.canyon_scroll = snap.canyon_scroll;
                self.score = snap.score;
                self.credits = snap.credits;
                self.wave_number = snap.wave_number;
                self.in_play = self.player_hull.intact_lobe_count() > 0;
                self.sync_all_bodies_to_lattice();
                return true;
            }
        }
        false
    }

    /// Stamp canyon walls, ground infrastructure, player Tri-Lobe hull, asteroids, aliens, and power cubes into the lattice.
    pub fn sync_all_bodies_to_lattice(&mut self) {
        self.solver.material.density.fill(1.0);
        self.solver.material.bulk_modulus.fill(1.0);
        self.solver.material.damping.fill(0.018);
        self.solver.material.cohesion.fill(0.0);
        self.solver.material.yield_pressure.fill(10.0);
        self.solver.material.elevation.fill(0.0);
        self.solver.boundary.clear_interior();

        stamp_canyon_into_lattice(
            &mut self.solver.material,
            &mut self.solver.boundary,
            self.canyon_scroll,
        );

        // Stamp ground structures into material impedance & 2.5D topographic elevation field
        for gt in &self.ground_targets {
            if gt.hull.y + gt.hull.radius >= 1.0
                && gt.hull.y - gt.hull.radius < self.height as f32 - 1.0
            {
                let r_b = (gt.hull.radius.ceil() as isize) + 1;
                let cx = gt.hull.x.round() as isize;
                let cy = gt.hull.y.round() as isize;
                let center = (gt.hull.local_size as f32 - 1.0) * 0.5;
                for oy in -r_b..=r_b {
                    for ox in -r_b..=r_b {
                        let px = cx + ox;
                        let py = cy + oy;
                        if px < 1
                            || px >= self.width as isize - 1
                            || py < 1
                            || py >= self.height as isize - 1
                        {
                            continue;
                        }
                        let coh = gt.hull.sample_local_cohesion(
                            (px as f32 - gt.hull.x) + center,
                            (py as f32 - gt.hull.y) + center,
                        );
                        if coh >= 0.15 {
                            let idx = self.solver.material.config.index(px as usize, py as usize);
                            self.solver.material.density[idx] = gt.hull.material.rho.max(1.0);
                            self.solver.material.bulk_modulus[idx] = gt.hull.material.bulk_modulus;
                            self.solver.material.damping[idx] = gt.hull.material.damping;
                            if let Some((bldg_h, ..)) =
                                sample_building_height_and_normal(gt, px as f32, py as f32)
                            {
                                self.solver.material.elevation[idx] =
                                    self.solver.material.elevation[idx].max(bldg_h);
                            }
                        }
                    }
                }
            }
        }

        // Single-pass O(cells) horizon shadow propagation over 3D building elevations
        self.solver.material.recompute_horizon_shadows(0.11);

        // Stamp player hull into material impedance field without locking interior wall_east/north faces,
        // preventing trapped wave rings around the player ship during normal movement.
        if self.in_play {
            let r_b = (self.player_hull.radius.ceil() as isize) + 1;
            let cx = self.player_hull.x.round() as isize;
            let cy = self.player_hull.y.round() as isize;
            let center = (self.player_hull.local_size as f32 - 1.0) * 0.5;
            for oy in -r_b..=r_b {
                for ox in -r_b..=r_b {
                    let px = cx + ox;
                    let py = cy + oy;
                    if px < 1
                        || px >= self.width as isize - 1
                        || py < 1
                        || py >= self.height as isize - 1
                    {
                        continue;
                    }
                    let coh = self.player_hull.sample_local_cohesion(
                        (px as f32 - self.player_hull.x) + center,
                        (py as f32 - self.player_hull.y) + center,
                    );
                    if coh >= 0.15 {
                        let idx = self.solver.material.config.index(px as usize, py as usize);
                        self.solver.material.density[idx] = self.player_hull.material.rho.max(1.0);
                        self.solver.material.bulk_modulus[idx] =
                            self.player_hull.material.bulk_modulus;
                        self.solver.material.damping[idx] = self.player_hull.material.damping;
                    }
                }
            }
        }
        for rock in &self.asteroids {
            if rock.y + rock.radius >= 1.0 && rock.y - rock.radius < self.height as f32 - 1.0 {
                rock.rasterize_to_lattice(
                    &mut self.solver.material,
                    &mut self.solver.boundary,
                    false,
                );
            }
        }
        for alien in &self.aliens {
            if alien.hull.y + alien.hull.radius >= 1.0
                && alien.hull.y - alien.hull.radius < self.height as f32 - 1.0
            {
                alien.hull.rasterize_to_lattice(
                    &mut self.solver.material,
                    &mut self.solver.boundary,
                    false,
                );
            }
        }
        for cube in &self.power_cubes {
            if cube.body.y + cube.body.radius >= 1.0
                && cube.body.y - cube.body.radius < self.height as f32 - 1.0
            {
                cube.body.rasterize_to_lattice(
                    &mut self.solver.material,
                    &mut self.solver.boundary,
                    false,
                );
            }
        }
    }

    /// Advance one macro-step of the Crystalline Corridor ecology.
    pub fn step(&mut self) -> ShmupEvent {
        let mut event = ShmupEvent::None;
        let sub_steps = 4;
        let dt_sub = 1.0 / (sub_steps as f32);

        if self.autopilot_enabled {
            self.step_autopilot();
        }

        if self.invulnerable_steps > 0 {
            self.invulnerable_steps -= 1;
        }
        if self.shield_visual_timer > 0 {
            self.shield_visual_timer -= 1;
        }
        if self.thruster_visual_timer > 0 {
            self.thruster_visual_timer -= 1;
        }
        if self.pipeline_flash_timer > 0 {
            self.pipeline_flash_timer -= 1;
        }
        self.canyon_scroll += self.scroll_speed;
        self.spawn_timer = self.spawn_timer.wrapping_add(1);

        if self.in_play
            && (self.asteroids.len() + self.aliens.len() + self.ground_targets.len() < 3
                || self.spawn_timer >= 240)
        {
            self.spawn_timer = 0;
            self.wave_number += 1;
            let w = self.wave_number;
            self.spawn_wave_formation(w);
        }

        // 1. Chladni Swarm Matriarchs radiate standing-wave pilot fields (`ChladniCarrierEmitter`),
        //    and Bohmian Swarm Drones surf the resulting `-grad Q` nodal valleys.
        let matriarch_positions: Vec<(u32, f32, f32, f32)> = self
            .aliens
            .iter()
            .filter_map(|a| {
                if a.role == AlienRole::ChladniMatriarch {
                    Some((a.hull.id, a.hull.x, a.hull.y, a.carrier_phase))
                } else {
                    None
                }
            })
            .collect();

        for &(_m_id, mx, my, phase) in &matriarch_positions {
            if self.wave_force_enabled && my > -12.0 && my < self.height as f32 - 4.0 {
                let emit_y = my.clamp(7.0, self.height as f32 - 5.0);
                let emitter = ChladniCarrierEmitter::new(mx, emit_y, 20.0, 0.48, 3.0, phase, 0.18);
                emitter.inject(&mut self.solver.state, false);
            }
        }

        let mut new_alien_bolts = Vec::new();
        let px = self.player_hull.x;
        let py = self.player_hull.y;
        let cube_positions: Vec<(f32, f32)> = self
            .power_cubes
            .iter()
            .filter(|c| c.body.y > 0.0)
            .map(|c| (c.body.x, c.body.y))
            .collect();

        for alien in &mut self.aliens {
            alien.carrier_phase = (alien.carrier_phase + 0.14).rem_euclid(TAU);
            let ax = alien.hull.x;
            let ay = alien.hull.y;
            let (q_fx, q_fy) = if ay >= 2.0 && ay < self.height as f32 - 3.0 {
                let gx = (ax.round() as usize).clamp(2, self.width - 3);
                let gy = (ay.round() as usize).clamp(2, self.height - 3);
                self.solver.state.quantum_force_at(gx, gy)
            } else {
                (0.0, 0.0)
            };

            let is_damaged = alien.hull.mean_cohesion() < 0.92;
            let nearest_cube = if is_damaged {
                cube_positions.iter().copied().min_by(|a, b| {
                    let da = (a.0 - ax).powi(2) + (a.1 - ay).powi(2);
                    let db = (b.0 - ax).powi(2) + (b.1 - ay).powi(2);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                })
            } else {
                None
            };

            match alien.role {
                AlienRole::ChladniMatriarch | AlienRole::SectorDreadnought => {
                    if let Some((cx, cy)) = nearest_cube {
                        let dx = cx - ax;
                        let dy = cy - ay;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        alien.hull.vx =
                            (alien.hull.vx + (dx / dist) * 0.018 + q_fx * 0.015).clamp(-0.36, 0.36);
                        alien.hull.vy =
                            (alien.hull.vy * 0.90 + (dy / dist) * 0.015 + 0.012).clamp(-0.22, 0.30);
                    } else {
                        let center_pull = (self.width as f32 * 0.5 - ax) * 0.002;
                        alien.hull.vx =
                            (alien.hull.vx * 0.95 + center_pull + q_fx * 0.015).clamp(-0.32, 0.32);
                        let target_vy = self.scroll_speed * 0.82;
                        alien.hull.vy =
                            (alien.hull.vy * 0.88 + target_vy * 0.12).clamp(-0.22, 0.28);
                    }
                }
                AlienRole::TwinGunship => {
                    let track_dx = (px - ax).clamp(-24.0, 24.0) * 0.003;
                    alien.hull.vx =
                        (alien.hull.vx * 0.94 + track_dx + q_fx * 0.012).clamp(-0.42, 0.42);
                    let target_vy = self.scroll_speed * 0.90;
                    alien.hull.vy = (alien.hull.vy * 0.88 + target_vy * 0.12).clamp(-0.24, 0.30);
                }
                AlienRole::BohmianSwarmDrone {
                    matriarch_id,
                    nodal_slot,
                } => {
                    if let Some(&(_, mx, my, phase)) =
                        matriarch_positions.iter().find(|&&(id, ..)| id == matriarch_id)
                    {
                        let slot_ang = FRAC_PI_2
                            + (nodal_slot as f32 - 1.0) * 0.75
                            + (phase * 0.25).sin() * 0.25;
                        let node_x = mx + slot_ang.cos() * 14.0;
                        let node_y = my + slot_ang.sin() * 12.0;
                        let ndx = node_x - ax;
                        let ndy = node_y - ay;
                        alien.hull.vx =
                            (alien.hull.vx * 0.90 + ndx * 0.025 + q_fx * 0.045).clamp(-0.42, 0.42);
                        alien.hull.vy =
                            (alien.hull.vy * 0.90 + ndy * 0.025 + q_fy * 0.045).clamp(-0.25, 0.42);
                    } else if let Some((cx, cy)) = nearest_cube {
                        let dx = cx - ax;
                        let dy = cy - ay;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        alien.hull.vx =
                            (alien.hull.vx + (dx / dist) * 0.022 + q_fx * 0.05).clamp(-0.42, 0.42);
                        alien.hull.vy =
                            (alien.hull.vy + (dy / dist) * 0.022 + q_fy * 0.05).clamp(-0.25, 0.42);
                    } else {
                        alien.hull.vx = (alien.hull.vx + q_fx * 0.05).clamp(-0.38, 0.38);
                        alien.hull.vy = (alien.hull.vy + 0.010 + q_fy * 0.04).clamp(-0.10, 0.38);
                    }
                }
                AlienRole::AnnealingHunter => {
                    if let Some((cx, cy)) = nearest_cube.or_else(|| cube_positions.first().copied())
                    {
                        let dx = cx - ax;
                        let dy = cy - ay;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        alien.hull.vx =
                            (alien.hull.vx + (dx / dist) * 0.025 + q_fx * 0.03).clamp(-0.40, 0.40);
                        alien.hull.vy =
                            (alien.hull.vy + (dy / dist) * 0.025 + q_fy * 0.03).clamp(-0.25, 0.40);
                    }
                }
            }

            // Attitude stabilization vs. asymmetrical thruster careening when fractured/cleaved
            if alien.cleave_count > 0 {
                // Missing a wing/nacelle causes asymmetrical thrust drift and yaw!
                let spin_dir = if alien.hull.omega >= 0.0 { 1.0 } else { -1.0 };
                alien.hull.omega = (alien.hull.omega * 0.96 + spin_dir * 0.004).clamp(-0.14, 0.14);
            } else {
                // Intact attitude thrusters gently right the ship after bullet torque kicks
                let mut angle_err = FRAC_PI_2 - alien.hull.theta;
                if angle_err > std::f32::consts::PI {
                    angle_err -= TAU;
                } else if angle_err < -std::f32::consts::PI {
                    angle_err += TAU;
                }
                alien.hull.omega = (alien.hull.omega * 0.86 + angle_err * 0.045).clamp(-0.15, 0.15);
            }

            if alien.fire_cooldown > 0 {
                alien.fire_cooldown -= 1;
            } else if self.in_play && ay >= 6.0 && ay < py - 14.0 {
                let dx = px - ax;
                let dy = py - ay;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let bolt_speed = 1.45;
                let bvx = (dx / dist) * bolt_speed;
                let bvy = ((dy / dist) * bolt_speed).max(0.65);

                if matches!(alien.role, AlienRole::TwinGunship | AlienRole::SectorDreadnought)
                    && alien.cleave_count == 0
                {
                    for side in [-3.2f32, 3.2f32] {
                        new_alien_bolts.push(SolitonBolt {
                            x: (ax + side).clamp(6.0, self.width as f32 - 6.0),
                            y: ay + alien.hull.radius + 1.5,
                            vx: bvx * 0.5,
                            vy: bolt_speed,
                            radius: 1.55,
                            ttl: 115,
                            from_player: false,
                            overcharged: false,
                            kind: ProjectileKind::EnemyOrb,
                        });
                    }
                } else {
                    new_alien_bolts.push(SolitonBolt {
                        x: ax + (dx / dist) * (alien.hull.radius + 2.0),
                        y: ay + (dy / dist) * (alien.hull.radius + 2.0),
                        vx: bvx,
                        vy: bvy,
                        radius: 1.55,
                        ttl: 115,
                        from_player: false,
                        overcharged: false,
                        kind: ProjectileKind::EnemyOrb,
                    });
                }
                alien.fire_cooldown = match alien.role {
                    AlienRole::SectorDreadnought => 64,
                    AlienRole::ChladniMatriarch => 88,
                    AlienRole::TwinGunship => 78,
                    _ => 125,
                };
            }
        }

        // Ground Turrets and Canal Patrol Boats fire flak & radiate canal wakes
        for gt in &mut self.ground_targets {
            if gt.kind == GroundTargetKind::CanalPatrolBoat
                && self.wave_force_enabled
                && gt.hull.y >= 4.0
                && gt.hull.y < self.height as f32 - 4.0
                && self.spawn_timer % 6 == 0
            {
                let plume = ReactionThrusterPlume::new(
                    gt.hull.x,
                    (gt.hull.y - 3.5).max(3.0),
                    1.1,
                    0.55,
                    -FRAC_PI_2,
                );
                let _ = plume.inject(&mut self.solver.state, &mut self.solver.material, false);
            }

            if matches!(
                gt.kind,
                GroundTargetKind::FlakTurret | GroundTargetKind::CanalPatrolBoat
            ) {
                if gt.fire_cooldown > 0 {
                    gt.fire_cooldown -= 1;
                } else if self.in_play && gt.hull.y >= 10.0 && gt.hull.y < py - 18.0 {
                    let dx = px - gt.hull.x;
                    let dy = py - gt.hull.y;
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    let speed = 1.25;
                    new_alien_bolts.push(SolitonBolt {
                        x: gt.hull.x + (dx / dist) * (gt.hull.radius + 1.8),
                        y: gt.hull.y + (dy / dist) * (gt.hull.radius + 1.8),
                        vx: (dx / dist) * speed,
                        vy: ((dy / dist) * speed).max(0.55),
                        radius: 1.45,
                        ttl: 105,
                        from_player: false,
                        overcharged: false,
                        kind: ProjectileKind::EnemyOrb,
                    });
                    gt.fire_cooldown = 115;
                }
            }
        }
        self.bolts.extend(new_alien_bolts);

        for crater in &mut self.craters {
            crater.y += self.scroll_speed;
            crater.ember_heat = (crater.ember_heat * 0.992).max(0.05);
        }
        self.craters.retain(|c| c.y - c.radius < self.height as f32 + 4.0);

        for gw in &mut self.grounded_wreckage {
            gw.body.y += self.scroll_speed;
            gw.skid_start_y += self.scroll_speed;
            gw.ember_heat = (gw.ember_heat * 0.990).max(0.08);
        }
        self.grounded_wreckage
            .retain(|gw| gw.body.y - gw.body.radius < self.height as f32 + 4.0);

        // 2. Substep kinematics, canyon wall containment, and impact resolution (`combat.rs`)
        for _ in 0..sub_steps {
            let (left_wall_p, right_wall_p, _) = canyon_wall_bounds(
                (self.player_hull.y.round() as usize).clamp(0, self.height - 1),
                self.height,
                self.width,
                self.canyon_scroll,
            );
            let min_px = left_wall_p as f32 + self.player_hull.radius + 1.5;
            let max_px = right_wall_p as f32 - self.player_hull.radius - 1.5;
            self.player_hull.x =
                (self.player_hull.x + self.player_hull.vx * dt_sub).clamp(min_px, max_px);
            self.player_hull.y = (self.player_hull.y + self.player_hull.vy * dt_sub).clamp(
                self.player_hull.radius + 12.0,
                self.height as f32 - self.player_hull.radius - 5.0,
            );

            for rock in &mut self.asteroids {
                rock.x += rock.vx * dt_sub;
                rock.y += rock.vy * dt_sub;
                rock.theta = (rock.theta + rock.omega * dt_sub).rem_euclid(TAU);

                let ry_idx = (rock.y.max(0.0).round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) =
                    canyon_wall_bounds(ry_idx, self.height, self.width, self.canyon_scroll);
                if rock.x < lw as f32 + rock.radius + 1.0 {
                    rock.x = lw as f32 + rock.radius + 1.0;
                    rock.vx = rock.vx.abs() * 0.85;
                } else if rock.x > rw as f32 - rock.radius - 1.0 {
                    rock.x = rw as f32 - rock.radius - 1.0;
                    rock.vx = -rock.vx.abs() * 0.85;
                }
            }

            for alien in &mut self.aliens {
                alien.hull.x += alien.hull.vx * dt_sub;
                alien.hull.y += alien.hull.vy * dt_sub;
                alien.hull.theta = (alien.hull.theta + alien.hull.omega * dt_sub).rem_euclid(TAU);
                let ay_idx = (alien.hull.y.max(0.0).round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) =
                    canyon_wall_bounds(ay_idx, self.height, self.width, self.canyon_scroll);
                let min_ax = lw as f32 + alien.hull.radius + 2.0;
                let max_ax = rw as f32 - alien.hull.radius - 2.0;
                if alien.hull.x < min_ax {
                    alien.hull.x = min_ax;
                    alien.hull.vx = alien.hull.vx.abs();
                } else if alien.hull.x > max_ax {
                    alien.hull.x = max_ax;
                    alien.hull.vx = -alien.hull.vx.abs();
                }
            }

            for gt in &mut self.ground_targets {
                gt.hull.x += gt.hull.vx * dt_sub;
                gt.hull.y += gt.hull.vy * dt_sub;
                if gt.kind == GroundTargetKind::CanalPatrolBoat {
                    let mid = self.width as f32 * 0.5;
                    if (gt.hull.x - mid).abs() > 16.0 {
                        gt.hull.vx = -gt.hull.vx;
                    }
                }
            }

            for cube in &mut self.power_cubes {
                cube.body.x += cube.body.vx * dt_sub;
                cube.body.y += cube.body.vy * dt_sub;
                cube.body.theta = (cube.body.theta + cube.body.omega * dt_sub).rem_euclid(TAU);

                let cy_idx = (cube.body.y.max(0.0).round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) =
                    canyon_wall_bounds(cy_idx, self.height, self.width, self.canyon_scroll);
                if cube.body.x < lw as f32 + 3.0 {
                    cube.body.x = lw as f32 + 3.0;
                    cube.body.vx = cube.body.vx.abs();
                } else if cube.body.x > rw as f32 - 3.0 {
                    cube.body.x = rw as f32 - 3.0;
                    cube.body.vx = -cube.body.vx.abs();
                }
            }

            self.step_bolts_and_impacts(dt_sub, &mut event);
        }

        // 3. Power Cube Capture & Exciton Annealing (contested between Player and Alien ships!)
        let mut kept_cubes = Vec::with_capacity(self.power_cubes.len());
        for mut cube in std::mem::take(&mut self.power_cubes) {
            cube.ttl = cube.ttl.saturating_sub(1);
            if cube.ttl == 0 || cube.body.y >= self.height as f32 - 3.0 {
                continue;
            }

            let pdx = self.player_hull.x - cube.body.x;
            let pdy = self.player_hull.y - cube.body.y;
            let p_dist_sq = pdx * pdx + pdy * pdy;

            if self.in_play && p_dist_sq < 22.0 * 22.0 && p_dist_sq > 1.0 {
                let d = p_dist_sq.sqrt();
                cube.body.vx = (cube.body.vx + (pdx / d) * 0.035).clamp(-0.48, 0.48);
                cube.body.vy = (cube.body.vy + (pdy / d) * 0.035).clamp(-0.48, 0.48);
            }

            if self.in_play
                && p_dist_sq <= (self.player_hull.radius + cube.body.radius + 1.8).powi(2)
            {
                let healed = self.player_hull.anneal_cells(cube.annealing_energy);
                self.capacitor = (self.capacitor + 22.0).min(self.max_capacitor);
                self.cubes_collected_player += 1;
                self.score += 125;
                event = ShmupEvent::PlayerCubeAnnealed {
                    intact_lobes: self.player_hull.intact_lobe_count(),
                    healed_cells: healed,
                };
                continue;
            }

            let mut captured_by_alien = None;
            for (a_idx, alien) in self.aliens.iter_mut().enumerate() {
                if alien.hull.y < 0.0 {
                    continue;
                }
                let adx = alien.hull.x - cube.body.x;
                let ady = alien.hull.y - cube.body.y;
                if adx * adx + ady * ady <= (alien.hull.radius + cube.body.radius + 1.2).powi(2) {
                    let healed = alien.hull.anneal_cells(cube.annealing_energy);
                    alien.cubes_healed += 1;
                    captured_by_alien = Some((a_idx, alien.hull.id, healed));
                    break;
                }
            }
            if let Some((_idx, alien_id, healed_cells)) = captured_by_alien {
                self.cubes_collected_aliens += 1;
                event = ShmupEvent::AlienCubeAnnealed {
                    alien_id,
                    healed_cells,
                };
                continue;
            }

            kept_cubes.push(cube);
        }
        self.power_cubes = kept_cubes;

        self.asteroids
            .retain(|r| r.y >= -45.0 && r.y < self.height as f32 - 2.0);
        self.aliens
            .retain(|a| a.hull.y >= -45.0 && a.hull.y < self.height as f32 - 2.0);
        self.ground_targets
            .retain(|g| g.hull.y >= -45.0 && g.hull.y < self.height as f32 - 2.0);

        for bolt in &mut self.bolts {
            bolt.ttl = bolt.ttl.saturating_sub(1);
        }

        self.player_hull.vx *= 0.88;
        self.player_hull.vy *= 0.88;
        self.player_hull.theta *= 0.85;

        // 4. Synchronize all bodies into the lattice and execute WGPU FDTD + Zero-Readback Force Reduction
        self.sync_all_bodies_to_lattice();

        let f_substeps = self.solver.config.substeps_per_macro;
        let mut queries: Vec<GpuBodyQuery> = Vec::new();
        let mut rock_query_indices: Vec<usize> = Vec::new();
        let mut cube_query_indices: Vec<usize> = Vec::new();

        if self.wave_force_enabled {
            for (idx, rock) in self.asteroids.iter().enumerate() {
                if rock.y >= 2.0 && rock.y < self.height as f32 - 2.0 {
                    rock_query_indices.push(idx);
                    queries.push(GpuBodyQuery {
                        x: rock.x,
                        y: rock.y,
                        radius: rock.radius,
                        yield_pressure: rock.material.yield_pressure,
                    });
                }
            }
            for (idx, cube) in self.power_cubes.iter().enumerate() {
                if cube.body.y >= 2.0 && cube.body.y < self.height as f32 - 2.0 {
                    cube_query_indices.push(idx);
                    queries.push(GpuBodyQuery {
                        x: cube.body.x,
                        y: cube.body.y,
                        radius: cube.body.radius,
                        yield_pressure: cube.body.material.yield_pressure,
                    });
                }
            }
        }

        let reductions =
            self.gpu_accel
                .step_and_reduce(&mut self.solver, f_substeps, &queries);

        if self.wave_force_enabled {
            let n_rock_q = rock_query_indices.len();
            for (&r_idx, red) in rock_query_indices
                .iter()
                .zip(reductions.iter().take(n_rock_q))
            {
                let rock = &mut self.asteroids[r_idx];
                rock.vx = (rock.vx + red.fx / rock.mass).clamp(-0.55, 0.55);
                rock.vy = (rock.vy + red.fy / rock.mass).clamp(-0.20, 0.58);
                rock.omega = (rock.omega + red.tau_z / rock.inertia).clamp(-0.14, 0.14);
            }
            for (&c_idx, red) in cube_query_indices
                .iter()
                .zip(reductions.iter().skip(n_rock_q))
            {
                let cube = &mut self.power_cubes[c_idx];
                cube.body.vx = (cube.body.vx + red.fx / cube.body.mass).clamp(-0.48, 0.48);
                cube.body.vy = (cube.body.vy + red.fy / cube.body.mass + 0.004).clamp(-0.35, 0.48);
                cube.body.omega =
                    (cube.body.omega + red.tau_z / cube.body.inertia).clamp(-0.15, 0.15);
            }
        }

        // 5. Binaural Cockpit Sonification (cockpit faces upward: -FRAC_PI_2)
        self.last_audio_frame = self.sonifier.sample_full_field(
            &self.solver.state,
            &self.solver.material,
            self.player_hull.x,
            self.player_hull.y,
            self.player_hull.vx,
            self.player_hull.vy,
            -FRAC_PI_2,
            false,
        );
        if let Ok(mut guard) = self.shared_audio_frame.try_lock() {
            guard.merge_telemetry(&self.last_audio_frame);
        }

        self.field_energy = FieldEnergy::compute(&self.solver.state, &self.solver.material);
        if self.score > self.high_score {
            self.high_score = self.score;
        }

        event
    }
}
