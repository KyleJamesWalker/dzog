//! Crystalline Corridor (`crates/shmup`): Vertical Wave-Stream Shmup on the Dzog Lattice.
//!
//! Coordinate convention (matching Bevy's top-left texture UV mapping):
//! - `y = 0` is the **top** of the screen (where enemy swarms and asteroids spawn offscreen at `y < 0`
//!   and flow downward with `+vy > 0`).
//! - `y = height` is the **bottom** of the screen (where the player's Tri-Lobe Crystalline Ship
//!   is stationed at `y ~ height * 0.82`, pointing upward toward `-Y` and firing fast upward
//!   Soliton Bolts with `vy < 0`).

use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::{Arc, Mutex};
pub use dzog_bevy::{FieldViewMode, GpuFieldAccelerator};
use dzog_core::*;
use dzog_wgpu::GpuBodyQuery;

use crate::canyon::{canyon_wall_bounds, stamp_canyon_into_lattice};
use crate::entities::{AlienRole, AlienVessel, PowerCube, ShmupBranchSnapshot, ShmupEvent, SolitonBolt};

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

    /// Descending crystalline asteroids, Chladni alien swarms, and Annealing Power Cubes.
    pub asteroids: Vec<CohesiveBody>,
    pub aliens: Vec<AlienVessel>,
    pub power_cubes: Vec<PowerCube>,
    pub bolts: Vec<SolitonBolt>,
    pub next_body_id: u32,

    /// Downward (`+Y`) wave-stream scroll velocity and phase.
    pub scroll_speed: f32,
    pub canyon_scroll: f32,
    pub spawn_timer: u16,

    /// Binaural cockpit sonifier & shared lock-free audio telemetry frame.
    pub sonifier: BinauralCockpitSonifier,
    pub last_audio_frame: FieldAcousticFrame,
    pub shared_audio_frame: Arc<Mutex<FieldAcousticFrame>>,

    pub score: u32,
    pub high_score: u32,
    pub wave_number: u32,
    pub topological_cleaves: u32,
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

        // Player starts near the bottom of the screen (y = 82% of height), pointing upward (-Y)
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
            aliens: Vec::with_capacity(16),
            power_cubes: Vec::with_capacity(32),
            bolts: Vec::with_capacity(64),
            next_body_id: 10,
            scroll_speed: 0.16,
            canyon_scroll: 0.0,
            spawn_timer: 0,
            sonifier: BinauralCockpitSonifier::new(3.5, 512),
            last_audio_frame: FieldAcousticFrame::default(),
            shared_audio_frame: Arc::new(Mutex::new(FieldAcousticFrame::default())),
            score: 0,
            high_score: 0,
            wave_number: 1,
            topological_cleaves: 0,
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
            pixel_buffer: vec![0u8; width * height * 4],
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

    /// Reset after core cavitation or manual reset (`[R]`).
    pub fn reset(&mut self) {
        self.solver.state.clear();
        self.player_hull = CohesiveBody::new_trilobe_ship(
            1,
            self.width as f32 * 0.5,
            self.height as f32 * 0.82,
            6.5,
        );
        self.invulnerable_steps = 35;
        self.shield_visual_timer = 0;
        self.thruster_visual_timer = 0;
        self.capacitor = 35.0;
        self.asteroids.clear();
        self.aliens.clear();
        self.power_cubes.clear();
        self.bolts.clear();
        self.score = 0;
        self.wave_number = 1;
        self.topological_cleaves = 0;
        self.cubes_collected_player = 0;
        self.cubes_collected_aliens = 0;
        self.in_play = true;
        self.spawn_timer = 0;
        self.spawn_wave_formation(1);
        self.sync_all_bodies_to_lattice();
    }

    /// Spawn a descending wave of crystalline asteroids and a Chladni Pilot-Wave Alien Swarm
    /// **offscreen above the top horizon (`y < 0`)** so they smoothly flow down into the arena.
    pub fn spawn_wave_formation(&mut self, wave: u32) {
        let w = self.width as f32;

        // 1. Spawn a Chladni Swarm Matriarch offscreen at the top (`y < 0`), pointing downward (`+Y`, theta = PI/2)
        let matriarch_id = self.next_body_id;
        self.next_body_id += 1;
        let mx = (w * 0.5 + ((wave as f32 * 1.3).sin() * w * 0.18)).clamp(38.0, w - 38.0);
        let my = -10.0;
        let mut m_hull = CohesiveBody::new_weaver_ship(matriarch_id, mx, my, 6.2);
        m_hull.vy = self.scroll_speed * 0.92;
        m_hull.theta = FRAC_PI_2;
        self.aliens.push(AlienVessel {
            role: AlienRole::ChladniMatriarch,
            hull: m_hull,
            fire_cooldown: 65,
            carrier_phase: 0.0,
            cubes_healed: 0,
        });

        let drone_count = (2 + (wave as usize % 3)).min(4);
        for slot in 0..drone_count {
            let d_id = self.next_body_id;
            self.next_body_id += 1;
            let ang = FRAC_PI_2 + (slot as f32 - (drone_count as f32 - 1.0) * 0.5) * 0.72;
            let r_nodal = 14.0;
            let dx = (mx + ang.cos() * r_nodal).clamp(26.0, w - 26.0);
            let dy = my - 6.0 + ang.sin() * r_nodal * 0.5;
            let mut d_hull = CohesiveBody::new_saucer(d_id, dx, dy, 3.1);
            d_hull.vy = self.scroll_speed * 0.92;
            d_hull.theta = FRAC_PI_2;
            self.aliens.push(AlienVessel {
                role: AlienRole::BohmianSwarmDrone {
                    matriarch_id,
                    nodal_slot: slot as u8,
                },
                hull: d_hull,
                fire_cooldown: 55 + (slot as u16) * 22,
                carrier_phase: slot as f32 * 1.1,
                cubes_healed: 0,
            });
        }

        // 2. Spawn descending Crystalline Asteroids offscreen above the top horizon (`y < 0`)
        let classes = [
            MineralClass::PiezoQuartz,
            MineralClass::BismuthCrystal,
            MineralClass::ObsidianAnvil,
        ];
        let num_rocks = (3 + (wave as usize % 3)).min(5);
        for i in 0..num_rocks {
            let r_id = self.next_body_id;
            self.next_body_id += 1;
            let lane_frac = (i as f32 + 0.5) / (num_rocks as f32);
            let rx = (28.0 + lane_frac * (w - 56.0)).clamp(26.0, w - 26.0);
            let ry = -7.0 - (i as f32 * 9.0);
            let class = classes[(i + wave as usize) % classes.len()];
            let radius = if class == MineralClass::ObsidianAnvil { 6.2 } else { 5.4 };
            let mut rock = CohesiveBody::new_asteroid(
                r_id,
                rx,
                ry,
                radius,
                6 + (i % 3),
                100 + wave * 31 + i as u32 * 17,
                class,
            );
            rock.vx = ((i as f32 * 1.7 + wave as f32).sin()) * 0.08;
            rock.vy = self.scroll_speed + 0.05 + (i as f32 * 0.02);
            rock.omega = if i % 2 == 0 { 0.025 } else { -0.025 };
            self.asteroids.push(rock);
        }

        // 3. Spawn initial Annealing Power Cubes drifting down from the upper horizon
        for c in 0..2 {
            let cx = (w * 0.32 + c as f32 * w * 0.36).clamp(30.0, w - 30.0);
            let cy = -4.0 - c as f32 * 10.0;
            self.spawn_power_cube(cx, cy, 0.0, self.scroll_speed * 0.95);
        }
    }

    /// Spawn one Annealing Piezo-Condensate ("Power Cube") at `(x, y)` with initial velocity `(vx, vy)`.
    pub fn spawn_power_cube(&mut self, x: f32, y: f32, vx: f32, vy: f32) {
        if self.power_cubes.len() >= 28 {
            return;
        }
        let id = self.next_body_id;
        self.next_body_id += 1;
        let mut body = CohesiveBody::new_power_cube(id, x, y);
        body.vx = vx.clamp(-0.45, 0.45);
        body.vy = vy.clamp(-0.45, 0.45);
        self.power_cubes.push(PowerCube {
            body,
            ttl: 600,
            annealing_energy: 28.0,
        });
    }

    /// 4-way planar reaction thruster control (`WASD` or `Arrow Keys`).
    ///
    /// Convention:
    /// - `dir_x < 0` = Left (`-X`), `dir_x > 0` = Right (`+X`)
    /// - `dir_y < 0` = Up toward top of screen (`-Y`), `dir_y > 0` = Down toward bottom (`+Y`)
    pub fn thrust_planar(&mut self, dir_x: f32, dir_y: f32) {
        let mag = (dir_x * dir_x + dir_y * dir_y).sqrt();
        if mag < 1e-3 {
            return;
        }
        let nx = dir_x / mag;
        let ny = dir_y / mag;

        let (port_ok, _, star_ok) = self.player_hull.trilobe_status();
        let wing_factor = 0.72 + 0.14 * (f32::from(port_ok) + f32::from(star_ok));
        let accel = 0.062 * wing_factor;

        self.player_hull.vx = (self.player_hull.vx + nx * accel).clamp(-0.68, 0.68);
        self.player_hull.vy = (self.player_hull.vy + ny * accel).clamp(-0.62, 0.58);
        self.player_hull.theta = (nx * 0.22).clamp(-0.28, 0.28);
        self.thruster_visual_timer = 5;

        if self.wave_force_enabled {
            let exhaust_angle = (-ny).atan2(-nx);
            let nozzle_x = (self.player_hull.x - nx * 4.5).clamp(4.0, self.width as f32 - 5.0);
            let nozzle_y = (self.player_hull.y - ny * 4.5).clamp(4.0, self.height as f32 - 5.0);
            let plume = ReactionThrusterPlume::new(nozzle_x, nozzle_y, 1.35, 0.85, exhaust_angle);
            let _ = plume.inject(&mut self.solver.state, &mut self.solver.material, false);
        }
    }

    /// Fire fast upward (`-Y`) phase-locked Soliton Bolts (`Spacebar`).
    ///
    /// - Intact wings fire twin wing-mounted soliton streams alongside the core cannon.
    /// - Upward (`-vy`) surge velocity imparts a physical Doppler amplitude & speed boost.
    pub fn fire_soliton_cannons(&mut self, request_overcharge: bool) {
        if !self.in_play {
            return;
        }
        let overcharged = request_overcharge && self.capacitor >= 32.0;
        if overcharged {
            self.capacitor = (self.capacitor - 32.0).max(0.0);
        }

        let (port_ok, _, star_ok) = self.player_hull.trilobe_status();
        let forward_doppler = (-self.player_hull.vy).max(0.0) * 0.40;
        // Fast upward projectile velocity (vy < 0 moves toward top of screen)
        let bolt_vy = -(1.85 + forward_doppler + if overcharged { 0.35 } else { 0.0 }).min(2.35);
        let bolt_vx = self.player_hull.vx * 0.18;

        // Muzzles sit ahead of the upward-pointing nose (-Y) and wingtips
        let mut muzzles: Vec<(f32, f32)> = vec![(self.player_hull.x, self.player_hull.y - 5.2)];
        if port_ok {
            muzzles.push((self.player_hull.x - 4.2, self.player_hull.y - 2.5));
        }
        if star_ok {
            muzzles.push((self.player_hull.x + 4.2, self.player_hull.y - 2.5));
        }

        for (mx, my) in muzzles {
            let bx = mx.clamp(6.0, self.width as f32 - 6.0);
            let by = my.clamp(6.0, self.height as f32 - 6.0);
            let amp = if overcharged { 2.40 } else { 1.55 } + forward_doppler * 1.1;

            if self.wave_force_enabled {
                let mut packet = DirectionalWavePacket::new(bx, by, 1.35, amp, -FRAC_PI_2);
                packet.inject_wrapped(&mut self.solver.state, &self.solver.material, false);
            }

            self.bolts.push(SolitonBolt {
                x: bx,
                y: by,
                vx: bolt_vx,
                vy: bolt_vy,
                radius: if overcharged { 2.2 } else { 1.6 },
                ttl: 110,
                from_player: true,
                overcharged,
            });
        }

        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_player_laser(overcharged);
        }
    }

    /// Emit an Antiphase Shield & Bohmian Attractor Pulse (`Shift` or Right-Click).
    pub fn trigger_antiphase_attractor(&mut self) -> f32 {
        let shield = AntiphaseShieldPulse::new(
            self.player_hull.x,
            self.player_hull.y,
            1.8,
            11.0,
            0.88,
        );
        let mut harvested = shield.cancel_and_harvest(
            &mut self.solver.state,
            &self.solver.material,
            false,
        );

        let r_sq = 12.0 * 12.0;
        let sx = self.player_hull.x;
        let sy = self.player_hull.y;
        let mut parried = 0u32;
        self.bolts.retain(|b| {
            if !b.from_player {
                let dx = b.x - sx;
                let dy = b.y - sy;
                if dx * dx + dy * dy <= r_sq {
                    parried += 1;
                    return false;
                }
            }
            true
        });

        if parried > 0 {
            harvested += parried as f32 * 14.0;
            self.score += parried * 75;
        }

        let attractor = GaussianImpulse::new(sx, sy, 3.2, 1.15);
        attractor.inject(&mut self.solver.state);

        for cube in &mut self.power_cubes {
            let dx = sx - cube.body.x;
            let dy = sy - cube.body.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < 46.0 * 46.0 && dist_sq > 1.0 {
                let dist = dist_sq.sqrt();
                cube.body.vx = (cube.body.vx + (dx / dist) * 0.16).clamp(-0.55, 0.55);
                cube.body.vy = (cube.body.vy + (dy / dist) * 0.16).clamp(-0.55, 0.55);
            }
        }

        self.capacitor = (self.capacitor + harvested * 2.2).min(self.max_capacitor);
        self.shield_visual_timer = 14;
        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_shield_pulse(parried > 0);
        }
        harvested
    }

    /// Inflict one structural lobe hit on the player's Tri-Lobe Crystalline Ship.
    pub fn damage_player_lobe(&mut self, hit_x: f32) -> ShmupEvent {
        let (port_ok, core_ok, star_ok) = self.player_hull.trilobe_status();
        if !core_ok {
            self.in_play = false;
            return ShmupEvent::PlayerLobeCleaved { remaining_lobes: 0 };
        }

        let lobe_to_sever = if port_ok && star_ok {
            if hit_x <= self.player_hull.x { -1 } else { 1 }
        } else if port_ok {
            -1
        } else if star_ok {
            1
        } else {
            0
        };

        let (cx, cy) = self.player_hull.sever_trilobe_section(lobe_to_sever);
        if self.wave_force_enabled {
            let blast = GaussianImpulse::new(cx, cy, 2.5, 2.8);
            blast.inject(&mut self.solver.state);
        }

        let remaining = self.player_hull.intact_lobe_count();
        self.invulnerable_steps = 36;
        if remaining == 0 {
            self.in_play = false;
        }

        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_cavitation_break(0.0, 1.4);
        }

        ShmupEvent::PlayerLobeCleaved {
            remaining_lobes: remaining,
        }
    }

    /// Save a counterfactual situation branch (`[B]`).
    pub fn save_branch_checkpoint(&mut self) {
        self.saved_branch = Some(ShmupBranchSnapshot {
            checkpoint: Checkpoint::capture(&self.solver),
            player_hull: self.player_hull.clone(),
            capacitor: self.capacitor,
            asteroids: self.asteroids.clone(),
            aliens: self.aliens.clone(),
            power_cubes: self.power_cubes.clone(),
            bolts: self.bolts.clone(),
            canyon_scroll: self.canyon_scroll,
            score: self.score,
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
                self.aliens = snap.aliens.clone();
                self.power_cubes = snap.power_cubes.clone();
                self.bolts = snap.bolts.clone();
                self.canyon_scroll = snap.canyon_scroll;
                self.score = snap.score;
                self.wave_number = snap.wave_number;
                self.in_play = self.player_hull.intact_lobe_count() > 0;
                self.sync_all_bodies_to_lattice();
                return true;
            }
        }
        false
    }

    /// Stamp canyon walls, player Tri-Lobe hull, asteroids, aliens, and power cubes into the lattice.
    pub fn sync_all_bodies_to_lattice(&mut self) {
        self.solver.material.density.fill(1.0);
        self.solver.material.bulk_modulus.fill(1.0);
        self.solver.material.damping.fill(0.018);
        self.solver.material.cohesion.fill(0.0);
        self.solver.material.yield_pressure.fill(10.0);
        self.solver.boundary.clear_interior();

        stamp_canyon_into_lattice(
            &mut self.solver.material,
            &mut self.solver.boundary,
            self.canyon_scroll,
        );

        if self.in_play {
            self.player_hull.rasterize_to_lattice(
                &mut self.solver.material,
                &mut self.solver.boundary,
                false,
            );
        }
        for rock in &self.asteroids {
            if rock.y + rock.radius >= 1.0 && rock.y - rock.radius < self.height as f32 - 1.0 {
                rock.rasterize_to_lattice(&mut self.solver.material, &mut self.solver.boundary, false);
            }
        }
        for alien in &self.aliens {
            if alien.hull.y + alien.hull.radius >= 1.0
                && alien.hull.y - alien.hull.radius < self.height as f32 - 1.0
            {
                alien
                    .hull
                    .rasterize_to_lattice(&mut self.solver.material, &mut self.solver.boundary, false);
            }
        }
        for cube in &self.power_cubes {
            if cube.body.y + cube.body.radius >= 1.0
                && cube.body.y - cube.body.radius < self.height as f32 - 1.0
            {
                cube.body
                    .rasterize_to_lattice(&mut self.solver.material, &mut self.solver.boundary, false);
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
        self.canyon_scroll += self.scroll_speed;
        self.spawn_timer = self.spawn_timer.wrapping_add(1);

        // Continuous horizon reinforcements flowing in from offscreen top (`y < 0`)
        if self.in_play && (self.asteroids.len() + self.aliens.len() < 3 || self.spawn_timer >= 240) {
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
            if self.wave_force_enabled && my > 4.0 && my < self.height as f32 - 4.0 {
                let emitter = ChladniCarrierEmitter::new(mx, my, 20.0, 0.48, 3.0, phase, 0.18);
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
                AlienRole::ChladniMatriarch => {
                    if let Some((cx, cy)) = nearest_cube {
                        let dx = cx - ax;
                        let dy = cy - ay;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        alien.hull.vx = (alien.hull.vx + (dx / dist) * 0.018 + q_fx * 0.015).clamp(-0.28, 0.28);
                        alien.hull.vy = (alien.hull.vy + (dy / dist) * 0.015 + 0.004).clamp(-0.10, 0.28);
                    } else {
                        let center_pull = (self.width as f32 * 0.5 - ax) * 0.002;
                        alien.hull.vx = (alien.hull.vx * 0.96 + center_pull + q_fx * 0.015).clamp(-0.24, 0.24);
                        alien.hull.vy = (self.scroll_speed * 0.82).clamp(0.08, 0.25);
                    }
                }
                AlienRole::BohmianSwarmDrone {
                    matriarch_id,
                    nodal_slot,
                } => {
                    if let Some(&(_, mx, my, phase)) =
                        matriarch_positions.iter().find(|&&(id, ..)| id == matriarch_id)
                    {
                        let slot_ang = FRAC_PI_2 + (nodal_slot as f32 - 1.0) * 0.75 + (phase * 0.25).sin() * 0.25;
                        let node_x = mx + slot_ang.cos() * 14.0;
                        let node_y = my + slot_ang.sin() * 12.0;
                        let ndx = node_x - ax;
                        let ndy = node_y - ay;
                        alien.hull.vx = (alien.hull.vx * 0.90 + ndx * 0.025 + q_fx * 0.045).clamp(-0.42, 0.42);
                        alien.hull.vy = (alien.hull.vy * 0.90 + ndy * 0.025 + q_fy * 0.045).clamp(-0.25, 0.42);
                    } else if let Some((cx, cy)) = nearest_cube {
                        let dx = cx - ax;
                        let dy = cy - ay;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        alien.hull.vx = (alien.hull.vx + (dx / dist) * 0.022 + q_fx * 0.05).clamp(-0.42, 0.42);
                        alien.hull.vy = (alien.hull.vy + (dy / dist) * 0.022 + q_fy * 0.05).clamp(-0.25, 0.42);
                    } else {
                        alien.hull.vx = (alien.hull.vx + q_fx * 0.05).clamp(-0.38, 0.38);
                        alien.hull.vy = (alien.hull.vy + 0.010 + q_fy * 0.04).clamp(-0.10, 0.38);
                    }
                }
                AlienRole::AnnealingHunter => {
                    if let Some((cx, cy)) = nearest_cube.or_else(|| cube_positions.first().copied()) {
                        let dx = cx - ax;
                        let dy = cy - ay;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        alien.hull.vx = (alien.hull.vx + (dx / dist) * 0.025 + q_fx * 0.03).clamp(-0.40, 0.40);
                        alien.hull.vy = (alien.hull.vy + (dy / dist) * 0.025 + q_fy * 0.03).clamp(-0.25, 0.40);
                    }
                }
            }

            // Enemies fire fast downward (`+Y`) projectiles less frequently once inside the screen
            if alien.fire_cooldown > 0 {
                alien.fire_cooldown -= 1;
            } else if self.in_play && ay >= 6.0 && ay < py - 14.0 {
                let dx = px - ax;
                let dy = py - ay;
                let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                let bolt_speed = 1.45;
                let bvx = (dx / dist) * bolt_speed;
                let bvy = ((dy / dist) * bolt_speed).max(0.65);
                new_alien_bolts.push(SolitonBolt {
                    x: ax + (dx / dist) * (alien.hull.radius + 2.0),
                    y: ay + (dy / dist) * (alien.hull.radius + 2.0),
                    vx: bvx,
                    vy: bvy,
                    radius: 1.55,
                    ttl: 115,
                    from_player: false,
                    overcharged: false,
                });
                alien.fire_cooldown = match alien.role {
                    AlienRole::ChladniMatriarch => 88,
                    _ => 125,
                };
            }
        }
        self.bolts.extend(new_alien_bolts);

        // 2. Substep kinematics, canyon wall containment, bolt impacts, and Power Cube annealing
        for _ in 0..sub_steps {
            let (left_wall_p, right_wall_p, _) = canyon_wall_bounds(
                (self.player_hull.y.round() as usize).clamp(0, self.height - 1),
                self.height,
                self.width,
                self.canyon_scroll,
            );
            let min_px = left_wall_p as f32 + self.player_hull.radius + 1.5;
            let max_px = right_wall_p as f32 - self.player_hull.radius - 1.5;
            self.player_hull.x = (self.player_hull.x + self.player_hull.vx * dt_sub).clamp(min_px, max_px);
            self.player_hull.y = (self.player_hull.y + self.player_hull.vy * dt_sub)
                .clamp(self.player_hull.radius + 12.0, self.height as f32 - self.player_hull.radius - 5.0);

            for rock in &mut self.asteroids {
                rock.x += rock.vx * dt_sub;
                rock.y += rock.vy * dt_sub;
                rock.theta = (rock.theta + rock.omega * dt_sub).rem_euclid(TAU);

                let ry_idx = (rock.y.max(0.0).round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) = canyon_wall_bounds(ry_idx, self.height, self.width, self.canyon_scroll);
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
                let ay_idx = (alien.hull.y.max(0.0).round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) = canyon_wall_bounds(ay_idx, self.height, self.width, self.canyon_scroll);
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

            for cube in &mut self.power_cubes {
                cube.body.x += cube.body.vx * dt_sub;
                cube.body.y += cube.body.vy * dt_sub;
                cube.body.theta = (cube.body.theta + cube.body.omega * dt_sub).rem_euclid(TAU);

                let cy_idx = (cube.body.y.max(0.0).round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) = canyon_wall_bounds(cy_idx, self.height, self.width, self.canyon_scroll);
                if cube.body.x < lw as f32 + 3.0 {
                    cube.body.x = lw as f32 + 3.0;
                    cube.body.vx = cube.body.vx.abs();
                } else if cube.body.x > rw as f32 - 3.0 {
                    cube.body.x = rw as f32 - 3.0;
                    cube.body.vx = -cube.body.vx.abs();
                }
            }

            // Advance Soliton Bolts & check impacts
            let old_bolts = std::mem::take(&mut self.bolts);
            let mut kept_bolts = Vec::with_capacity(old_bolts.len());
            let mut rock_hits: Vec<(usize, f32, f32, f32, f32, bool)> = Vec::new();
            let mut alien_hits: Vec<(usize, f32, f32, f32, f32, bool)> = Vec::new();
            let mut player_hit_x: Option<f32> = None;

            for mut bolt in old_bolts {
                if bolt.ttl == 0 {
                    continue;
                }
                bolt.x += bolt.vx * dt_sub;
                bolt.y += bolt.vy * dt_sub;

                if bolt.x < 3.0
                    || bolt.x >= self.width as f32 - 3.0
                    || bolt.y < 2.0
                    || bolt.y >= self.height as f32 - 2.0
                {
                    continue;
                }

                // Canyon wall grazing reflection
                let by_idx = (bolt.y.round() as usize).clamp(0, self.height - 1);
                let (lw, rw, _) = canyon_wall_bounds(by_idx, self.height, self.width, self.canyon_scroll);
                if bolt.x <= lw as f32 + 1.5 {
                    bolt.x = lw as f32 + 2.0;
                    bolt.vx = bolt.vx.abs().max(0.32);
                    if self.wave_force_enabled {
                        GaussianImpulse::new(bolt.x, bolt.y, 1.6, 1.4).inject(&mut self.solver.state);
                    }
                } else if bolt.x >= rw as f32 - 1.5 {
                    bolt.x = rw as f32 - 2.0;
                    bolt.vx = -bolt.vx.abs().max(0.32);
                    if self.wave_force_enabled {
                        GaussianImpulse::new(bolt.x, bolt.y, 1.6, 1.4).inject(&mut self.solver.state);
                    }
                }

                let mut consumed = false;
                for (r_idx, rock) in self.asteroids.iter().enumerate() {
                    if rock.y + rock.radius < 0.0 {
                        continue;
                    }
                    let dx = bolt.x - rock.x;
                    let dy = bolt.y - rock.y;
                    if dx * dx + dy * dy <= (rock.radius + bolt.radius).powi(2) {
                        rock_hits.push((r_idx, bolt.x, bolt.y, bolt.vx, bolt.vy, bolt.overcharged));
                        consumed = true;
                        break;
                    }
                }

                if !consumed && bolt.from_player {
                    for (a_idx, alien) in self.aliens.iter().enumerate() {
                        if alien.hull.y + alien.hull.radius < 0.0 {
                            continue;
                        }
                        let dx = bolt.x - alien.hull.x;
                        let dy = bolt.y - alien.hull.y;
                        if dx * dx + dy * dy <= (alien.hull.radius + bolt.radius).powi(2) {
                            alien_hits.push((a_idx, bolt.x, bolt.y, bolt.vx, bolt.vy, bolt.overcharged));
                            consumed = true;
                            break;
                        }
                    }
                }

                if !consumed && !bolt.from_player && self.invulnerable_steps == 0 && self.in_play {
                    let dx = bolt.x - self.player_hull.x;
                    let dy = bolt.y - self.player_hull.y;
                    if dx * dx + dy * dy <= (self.player_hull.radius * 0.85 + bolt.radius).powi(2) {
                        player_hit_x = Some(bolt.x);
                        consumed = true;
                    }
                }

                if !consumed {
                    if self.wave_force_enabled {
                        let (c_amp, l_gain, c_sigma) = if bolt.overcharged {
                            (1.30, 0.92, 1.50)
                        } else {
                            (0.80, 0.62, 1.18)
                        };
                        SolitonCarrierPulse::new(
                            bolt.x,
                            bolt.y,
                            bolt.vx,
                            bolt.vy,
                            c_sigma,
                            c_amp,
                            l_gain,
                        )
                        .couple_to_lattice(&mut self.solver.state, &self.solver.material, false);
                    }
                    kept_bolts.push(bolt);
                }
            }
            self.bolts = kept_bolts;

            // Process asteroid impacts & spawn Annealing Power Cubes on shatter/cleave
            if !rock_hits.is_empty() {
                rock_hits.sort_by(|a, b| b.0.cmp(&a.0));
                rock_hits.dedup_by_key(|h| h.0);

                for (r_idx, hx, hy, bvx, bvy, overcharged) in rock_hits {
                    if r_idx >= self.asteroids.len() {
                        continue;
                    }
                    let speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                    let dir_x = bvx / speed;
                    let dir_y = bvy / speed;
                    let energy = if overcharged { 2.2 } else { 1.25 };

                    self.asteroids[r_idx].apply_contact_impulse(
                        hx,
                        hy,
                        dir_x * 1.3 * energy,
                        dir_y * 1.3 * energy,
                        None,
                    );
                    let cleaved = self.asteroids[r_idx]
                        .carve_cleavage_fissure(hx, hy, dir_x, dir_y, energy, None);
                    let parent_id = self.asteroids[r_idx].id;
                    let class = self.asteroids[r_idx].material.class;

                    if self.wave_force_enabled && hy >= 1.0 && hy < self.height as f32 - 1.0 {
                        GaussianImpulse::new(hx, hy, 2.0, 2.2 * energy).inject(&mut self.solver.state);
                    }

                    let children = self.asteroids[r_idx]
                        .split_connected_components(&mut self.next_body_id, None);
                    if children.len() >= 2 {
                        let rx = self.asteroids[r_idx].x;
                        let ry = self.asteroids[r_idx].y;
                        let rvy = self.asteroids[r_idx].vy.abs().max(self.scroll_speed * 0.8);
                        self.asteroids.swap_remove(r_idx);
                        let num_c = children.len();
                        self.asteroids.extend(children);
                        self.topological_cleaves += 1;
                        self.score += 180 * num_c as u32;
                        self.spawn_power_cube(rx, ry, dir_x * 0.12, rvy);
                        event = ShmupEvent::AsteroidCleaved {
                            parent_id,
                            children_spawned: num_c,
                        };
                    } else if self.asteroids[r_idx].active_cell_count() < 10
                        || (cleaved && self.asteroids[r_idx].radius < 4.5)
                    {
                        let removed = self.asteroids.swap_remove(r_idx);
                        let rvy = removed.vy.abs().max(self.scroll_speed * 0.8);
                        self.score += 250;
                        self.spawn_power_cube(removed.x - 1.8, removed.y, -0.14, rvy);
                        self.spawn_power_cube(removed.x + 1.8, removed.y, 0.14, rvy);
                        event = ShmupEvent::AsteroidShattered {
                            body_id: parent_id,
                            cubes_spawned: 2,
                        };
                    } else {
                        self.score += 45;
                        event = ShmupEvent::AsteroidCracked {
                            body_id: parent_id,
                            class,
                        };
                    }
                }
            }

            // Process alien hits & spawn Power Cubes on destruction
            if !alien_hits.is_empty() {
                alien_hits.sort_by(|a, b| b.0.cmp(&a.0));
                alien_hits.dedup_by_key(|h| h.0);

                for (a_idx, hx, hy, bvx, bvy, overcharged) in alien_hits {
                    if a_idx >= self.aliens.len() {
                        continue;
                    }
                    let speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                    let dir_x = bvx / speed;
                    let dir_y = bvy / speed;
                    let energy = if overcharged { 2.3 } else { 1.4 };

                    self.aliens[a_idx].hull.apply_contact_impulse(
                        hx,
                        hy,
                        dir_x * 1.4 * energy,
                        dir_y * 1.4 * energy,
                        None,
                    );
                    let cleaved = self.aliens[a_idx]
                        .hull
                        .carve_cleavage_fissure(hx, hy, dir_x, dir_y, energy, None);

                    if cleaved
                        || self.aliens[a_idx].hull.mean_cohesion() < 0.62
                        || self.aliens[a_idx].hull.active_cell_count() < 12
                    {
                        let dead = self.aliens.swap_remove(a_idx);
                        self.score += match dead.role {
                            AlienRole::ChladniMatriarch => 600,
                            _ => 300,
                        };
                        if self.wave_force_enabled && dead.hull.y >= 1.0 {
                            GaussianImpulse::new(dead.hull.x, dead.hull.y, 2.8, 3.2)
                                .inject(&mut self.solver.state);
                        }
                        let cubes_n = if dead.role == AlienRole::ChladniMatriarch { 3 } else { 1 };
                        for k in 0..cubes_n {
                            let off = (k as f32 - 0.5) * 2.2;
                            self.spawn_power_cube(
                                dead.hull.x + off,
                                dead.hull.y.max(2.0),
                                off * 0.08,
                                self.scroll_speed * 0.9,
                            );
                        }
                        event = ShmupEvent::AlienDestroyed {
                            role: dead.role,
                            cubes_spawned: cubes_n,
                        };
                    }
                }
            }

            // Check asteroid/alien hull collision with player ship
            if player_hit_x.is_none() && self.invulnerable_steps == 0 && self.in_play {
                for rock in &self.asteroids {
                    let dx = rock.x - self.player_hull.x;
                    let dy = rock.y - self.player_hull.y;
                    if dx * dx + dy * dy <= (rock.radius + self.player_hull.radius * 0.72).powi(2) {
                        player_hit_x = Some(rock.x);
                        break;
                    }
                }
            }

            if let Some(hx) = player_hit_x {
                event = self.damage_player_lobe(hx);
            }
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

            // Gentle passive Bohmian pull when a Power Cube enters close range of player hull
            if self.in_play && p_dist_sq < 22.0 * 22.0 && p_dist_sq > 1.0 {
                let d = p_dist_sq.sqrt();
                cube.body.vx = (cube.body.vx + (pdx / d) * 0.035).clamp(-0.48, 0.48);
                cube.body.vy = (cube.body.vy + (pdy / d) * 0.035).clamp(-0.48, 0.48);
            }

            if self.in_play && p_dist_sq <= (self.player_hull.radius + cube.body.radius + 1.8).powi(2) {
                let healed = self.player_hull.anneal_cells(cube.annealing_energy);
                self.capacitor = (self.capacitor + 22.0).min(self.max_capacitor);
                self.cubes_collected_player += 1;
                self.score += 125;
                if self.wave_force_enabled {
                    GaussianImpulse::new(self.player_hull.x, self.player_hull.y, 2.2, 1.2)
                        .inject(&mut self.solver.state);
                }
                event = ShmupEvent::PlayerCubeAnnealed {
                    intact_lobes: self.player_hull.intact_lobe_count(),
                    healed_cells: healed,
                };
                continue;
            }

            // Check if an Alien ship captures the Power Cube to heal its crystal fractures
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

        // Allow offscreen top spawns (`y >= -45.0`) to flow downward and cull once past bottom (`height - 2.0`)
        self.asteroids.retain(|r| r.y >= -45.0 && r.y < self.height as f32 - 2.0);
        self.aliens.retain(|a| a.hull.y >= -45.0 && a.hull.y < self.height as f32 - 2.0);

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

        let reductions = self
            .gpu_accel
            .step_and_reduce(&mut self.solver, f_substeps, &queries);

        if self.wave_force_enabled {
            let n_rock_q = rock_query_indices.len();
            for (&r_idx, red) in rock_query_indices.iter().zip(reductions.iter().take(n_rock_q)) {
                let rock = &mut self.asteroids[r_idx];
                rock.vx = (rock.vx + red.fx / rock.mass).clamp(-0.55, 0.55);
                rock.vy = (rock.vy + red.fy / rock.mass).clamp(-0.20, 0.58);
                rock.omega = (rock.omega + red.tau_z / rock.inertia).clamp(-0.14, 0.14);
            }
            for (&c_idx, red) in cube_query_indices.iter().zip(reductions.iter().skip(n_rock_q)) {
                let cube = &mut self.power_cubes[c_idx];
                cube.body.vx = (cube.body.vx + red.fx / cube.body.mass).clamp(-0.48, 0.48);
                cube.body.vy = (cube.body.vy + red.fy / cube.body.mass + 0.004).clamp(-0.35, 0.48);
                cube.body.omega = (cube.body.omega + red.tau_z / cube.body.inertia).clamp(-0.15, 0.15);
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

    /// Render the Crystalline Corridor field, metamaterial canyon walls, Chladni interference waves,
    /// Annealing Power Cubes, and the Tri-Lobe Player Ship into `self.pixel_buffer` (`RGBA8`).
    pub fn render_to_pixel_buffer(&mut self) {
        let w = self.width;
        let h = self.height;
        if self.pixel_buffer.len() != w * h * 4 {
            self.pixel_buffer.resize(w * h * 4, 0);
        }

        let has_lum = self.solver.state.luminescence.len() == w * h;
        let scroll_i = self.canyon_scroll.floor() as i32;

        // 1. Background wavefield, scrolling starfield, Bohmian Q ridges, and canyon walls
        for y in 0..h {
            let (left_edge, right_edge, canyon_class) =
                canyon_wall_bounds(y, h, w, self.canyon_scroll);
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let p = self.solver.state.pressure[idx];
                let vx = self.solver.state.vx[idx];
                let vy = self.solver.state.vy[idx];
                let lum = if has_lum { self.solver.state.luminescence[idx] } else { 0.0 };
                let rho = self.solver.material.density[idx];
                let k = self.solver.material.bulk_modulus[idx];
                let coh = self.solver.material.cohesion[idx];

                // Downward-scrolling starfield in the open corridor
                let sy_scroll = (y as i32 - scroll_i).rem_euclid(h as i32) as u32;
                let star_hash = (x as u32)
                    .wrapping_mul(374761393)
                    .wrapping_add(sy_scroll.wrapping_mul(668265263));
                let star = if (star_hash & 0x1FF) == 0 {
                    0.16 + ((star_hash >> 9) & 0x7) as f32 * 0.025
                } else {
                    0.0
                };

                let in_canyon = x < left_edge || x >= right_edge;
                let (mut r, mut g, mut b) = if in_canyon && self.view_mode == FieldViewMode::CompositeArcade {
                    let rim_dist = if x < left_edge {
                        (left_edge - x) as f32
                    } else {
                        (x - right_edge + 1) as f32
                    };
                    let rim_glow = (1.0 / (0.6 + rim_dist * 0.45)).clamp(0.0, 1.0);
                    let base = match canyon_class {
                        MineralClass::ObsidianAnvil => (0.09, 0.11, 0.18),
                        MineralClass::BismuthCrystal => (0.14, 0.11, 0.22),
                        _ => (0.08, 0.18, 0.24),
                    };
                    (
                        base.0 + rim_glow * 0.18 + lum * 0.25 + p.abs() * 0.22,
                        base.1 + rim_glow * 0.36 + lum * 0.45 + p.abs() * 0.28,
                        base.2 + rim_glow * 0.52 + lum * 0.60 + p.abs() * 0.35,
                    )
                } else {
                    match self.view_mode {
                        FieldViewMode::CompositeArcade => {
                            let p_pos = p.max(0.0);
                            let p_neg = (-p).max(0.0);
                            let v_mag = (vx * vx + vy * vy).sqrt();
                            let l_glow = lum.min(3.0);
                            (
                                0.015 + star * 0.8 + p_pos * 0.44 + v_mag * 0.15 + l_glow * 0.24,
                                0.024 + star * 0.9 + p_pos * 0.28 + p_neg * 0.38 + v_mag * 0.25 + l_glow * 0.62,
                                0.052 + star + p_neg * 0.64 + p_pos * 0.22 + l_glow * 0.84,
                            )
                        }
                        FieldViewMode::PressureAndBohmian => {
                            let q = self.solver.state.quantum_potential_at(x, y);
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
                            let v_mag = (vx * vx + vy * vy).sqrt();
                            let flux = (p * vx).hypot(p * vy);
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
                    }
                };

                let p_idx = (y * w + x) * 4;
                r = r.clamp(0.0, 1.0);
                g = g.clamp(0.0, 1.0);
                b = b.clamp(0.0, 1.0);
                self.pixel_buffer[p_idx] = (r * 255.0) as u8;
                self.pixel_buffer[p_idx + 1] = (g * 255.0) as u8;
                self.pixel_buffer[p_idx + 2] = (b * 255.0) as u8;
                self.pixel_buffer[p_idx + 3] = 255;
            }
        }

        // 2. Render Asteroids (color-coded by mineralogical class)
        for rock in &self.asteroids {
            let tint = match rock.material.class {
                MineralClass::PiezoQuartz => (0.28, 0.82, 0.96),
                MineralClass::ObsidianAnvil => (0.45, 0.50, 0.68),
                MineralClass::BismuthCrystal => (0.78, 0.48, 0.92),
                MineralClass::AlienResonator => (0.35, 0.88, 0.65),
            };
            Self::draw_cohesive_body(&mut self.pixel_buffer, w, h, rock, tint, false);
        }

        // 3. Render Alien Swarm Matriarchs & Bohmian Drones
        for alien in &self.aliens {
            let tint = match alien.role {
                AlienRole::ChladniMatriarch => (0.95, 0.36, 0.78),
                AlienRole::BohmianSwarmDrone { .. } => (0.98, 0.55, 0.22),
                AlienRole::AnnealingHunter => (0.92, 0.25, 0.35),
            };
            Self::draw_cohesive_body(&mut self.pixel_buffer, w, h, &alien.hull, tint, false);
        }

        // 4. Render Annealing Piezo Power-Cubes (bright emerald/gold crystalline condensates + halo)
        for cube in &self.power_cubes {
            Self::draw_cohesive_body(
                &mut self.pixel_buffer,
                w,
                h,
                &cube.body,
                (0.22, 0.98, 0.55),
                true,
            );
        }

        // 5. Render Player Tri-Lobe Crystalline Ship + aft thruster glow + cockpit canopy
        if self.in_play {
            let ship_tint = if self.shield_visual_timer > 0 {
                (0.55, 1.0, 1.0)
            } else if self.invulnerable_steps % 4 >= 2 {
                (0.85, 0.95, 1.0)
            } else {
                (0.20, 0.92, 0.98)
            };
            Self::draw_cohesive_body(
                &mut self.pixel_buffer,
                w,
                h,
                &self.player_hull,
                ship_tint,
                true,
            );

            // Highlight upward-facing cockpit spearhead (-Y) and aft plasma nozzle (+Y)
            let sx = self.player_hull.x.round() as isize;
            let sy = self.player_hull.y.round() as isize;
            for oy in -2..=0 {
                for ox in -1..=1 {
                    let px = sx + ox;
                    let py = sy + oy - 1;
                    if px >= 0 && px < w as isize && py >= 0 && py < h as isize {
                        let p_idx = (py as usize * w + px as usize) * 4;
                        self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&[230, 255, 255, 255]);
                    }
                }
            }
            if self.thruster_visual_timer > 0 {
                for oy in 4..=7 {
                    for ox in -1..=1 {
                        let px = sx + ox;
                        let py = sy + oy;
                        if px >= 0 && px < w as isize && py >= 0 && py < h as isize {
                            let p_idx = (py as usize * w + px as usize) * 4;
                            self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&[255, 195, 80, 255]);
                        }
                    }
                }
            }
        }

        // 6. Render Soliton Bolts with velocity-aligned luminous streaks
        for bolt in &self.bolts {
            let bx = bolt.x.round() as isize;
            let by = bolt.y.round() as isize;
            let r_b = bolt.radius.ceil() as isize;
            let col = if bolt.from_player {
                if bolt.overcharged {
                    [255, 245, 130]
                } else {
                    [120, 250, 255]
                }
            } else {
                [255, 95, 145]
            };

            for oy in -(r_b + 1)..=(r_b + 1) {
                for ox in -r_b..=r_b {
                    let px = bx + ox;
                    let py = by + oy;
                    if px < 0 || px >= w as isize || py < 0 || py >= h as isize {
                        continue;
                    }
                    if (ox * ox) as f32 + (oy as f32 * 0.65).powi(2) <= bolt.radius * bolt.radius {
                        let p_idx = (py as usize * w + px as usize) * 4;
                        self.pixel_buffer[p_idx] = col[0];
                        self.pixel_buffer[p_idx + 1] = col[1];
                        self.pixel_buffer[p_idx + 2] = col[2];
                    }
                }
            }
        }
    }

    fn draw_cohesive_body(
        pixel_buffer: &mut [u8],
        w: usize,
        h: usize,
        body: &CohesiveBody,
        tint: (f32, f32, f32),
        bright_rim: bool,
    ) {
        let r_bound = (body.radius.ceil() as isize) + 1;
        let cx = body.x.round() as isize;
        let cy = body.y.round() as isize;
        let cos_t = body.theta.cos();
        let sin_t = body.theta.sin();
        let center = (body.local_size as f32 - 1.0) * 0.5;

        for oy in -r_bound..=r_bound {
            for ox in -r_bound..=r_bound {
                let px = cx + ox;
                let py = cy + oy;
                if px < 0 || px >= w as isize || py < 0 || py >= h as isize {
                    continue;
                }
                let dx = px as f32 - body.x;
                let dy = py as f32 - body.y;
                let lx = dx * cos_t + dy * sin_t + center;
                let ly = -dx * sin_t + dy * cos_t + center;
                let coh = body.sample_local_cohesion(lx, ly);
                if coh < 0.15 {
                    continue;
                }

                let (elev, nx, ny, ridge) = body.sample_crystal_facet(lx, ly);
                let light = (nx * 0.35 - ny * 0.65).max(0.0);
                let crack_glow = if coh < 0.58 { (0.58 - coh) * 2.2 } else { 0.0 };
                let rim = if bright_rim { 0.25 } else { 0.10 };

                let r = (tint.0 * (0.38 + elev * 0.55 + light * 0.35) + ridge * 0.28 + crack_glow * 0.9 + rim)
                    .clamp(0.0, 1.0);
                let g = (tint.1 * (0.38 + elev * 0.55 + light * 0.35) + ridge * 0.32 + crack_glow * 0.4 + rim)
                    .clamp(0.0, 1.0);
                let b = (tint.2 * (0.38 + elev * 0.55 + light * 0.35) + ridge * 0.35 + crack_glow * 0.2 + rim)
                    .clamp(0.0, 1.0);

                let p_idx = (py as usize * w + px as usize) * 4;
                pixel_buffer[p_idx] = (r * 255.0) as u8;
                pixel_buffer[p_idx + 1] = (g * 255.0) as u8;
                pixel_buffer[p_idx + 2] = (b * 255.0) as u8;
            }
        }
    }
}
