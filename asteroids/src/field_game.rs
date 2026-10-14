//! Field-First Crystalline Asteroids & Alien Ecology on the Dzog Lattice.
//!
//! Demonstrates all five roadmap capabilities from `readme.md` and `philosophy.md`:
//! 1. Free-Rotating Cohesive Bodies & Topological Cleaving (`CohesiveBody`, BFS fracture splitting,
//!    off-center recoil torque, and mineralogical impedance contrasts).
//! 2. Toroidal Periodic Arena & Advected Open-Space Gravity Well Lattices (`[M]` mode toggle).
//! 3. Multi-Field Channels: Poisson Gravity Wells (`Phi_g`), Reaction Thruster Plumes (exhaust backblast),
//!    Field-Brake Bow Shocks, Phase-Locked Soliton Bolts, and Antiphase Interference Shields (`delta_phi = pi`).
//! 4. Zero-Readback GPU Force & Probe Reduction Pipeline (`reduce_bodies_gpu`).
//! 5. Live Binaural Cockpit Field Sonification (`BinauralCockpitSonifier` with Doppler telemetry).

use std::f32::consts::{FRAC_PI_2, PI, TAU};
pub use dzog_bevy::{FieldViewMode, GpuFieldAccelerator};
use dzog_core::*;
use dzog_wgpu::GpuBodyQuery;

#[path = "autopilot.rs"]
pub mod autopilot;

/// World lattice topology mode (`[M]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldTopologyMode {
    /// Classic toroidal periodic wrap (`OuterBoundaryRule::Periodic`).
    ToroidalPeriodic,
    /// Central singularity gravity well (`Phi_g`) with open-space absorbing sponge & camera advection.
    GravityWellArena,
}

#[allow(dead_code)]
impl WorldTopologyMode {
    pub fn next(self) -> Self {
        match self {
            Self::ToroidalPeriodic => Self::GravityWellArena,
            Self::GravityWellArena => Self::ToroidalPeriodic,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ToroidalPeriodic => "Toroidal Periodic",
            Self::GravityWellArena => "Gravity Well + Sponge",
        }
    }
}

/// Coherent plasma-soliton projectile coupled to the wavefield.
#[derive(Debug, Clone, PartialEq)]
pub struct SolitonBolt {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub radius: f32,
    /// Remaining lifetime in substeps.
    pub ttl: u16,
    /// True if fired by the player; false if fired by an alien antagonist.
    pub from_player: bool,
    /// True if supercharged via Doppler thrust or Hamiltonian Capacitor Phase-Lance (`>= 1.8` cleave energy).
    pub overcharged: bool,
}

/// Alien antagonist archetype coupled to the PDE field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlienKind {
    /// Surfs Bohmian quantum potential minima (-grad Q) and fires telegraphed soliton bolts.
    BohmianSaucer,
    /// Orbits asteroids and pumps a standing-wave resonance beam into their fissures.
    HarmonicWeaver,
}

/// Field-coupled alien enemy vessel.
#[derive(Debug, Clone, PartialEq)]
pub struct AlienEnemy {
    pub kind: AlienKind,
    /// Rotating destructible crystal hull stamped into the lattice.
    pub hull: CohesiveBody,
    /// Cooldown before next attack cycle.
    pub fire_cooldown: u16,
    /// Telegraph charge-up timer (0 = idle, 1..20 = visible standing-wave lock-on before firing).
    pub telegraph_timer: u16,
    /// Target aim direction during telegraph lock-on.
    pub aim_angle: f32,
}

/// Complete snapshot of an Asteroids game & field situation for counterfactual branching (`[B]`/`[U]`).
#[derive(Debug, Clone)]
pub struct AsteroidsBranchSnapshot {
    pub checkpoint: Checkpoint,
    pub ship_x: f32,
    pub ship_y: f32,
    pub ship_vx: f32,
    pub ship_vy: f32,
    pub ship_theta: f32,
    pub ship_omega: f32,
    pub capacitor: f32,
    pub asteroids: Vec<CohesiveBody>,
    pub aliens: Vec<AlienEnemy>,
    pub bolts: Vec<SolitonBolt>,
    pub lives: u32,
    pub score: u32,
    pub wave_number: u32,
    pub topology: WorldTopologyMode,
}

/// High-level simulation event emitted during a step.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsteroidEvent {
    None,
    /// Asteroid was cracked or glanced off (carving internal waveguides and imparting spin).
    AsteroidCracked { body_id: u32, class: MineralClass },
    /// Asteroid was topologically cleaved into child fragments.
    AsteroidCleaved { parent_id: u32, children_spawned: usize },
    /// Asteroid shard vaporized into a cavitation shockwave.
    AsteroidShattered { body_id: u32, remaining: usize },
    /// Alien enemy was cleaved or destroyed.
    AlienDestroyed { kind: AlienKind },
    /// Incoming wave/bolt was parried by the player's Antiphase Interference Shield.
    ShieldParry,
    /// Player ship was struck.
    ShipDamaged { remaining_lives: u32 },
    /// All asteroids and aliens cleared in the current wave.
    WaveCleared { next_wave: u32 },
    /// Player lost all lives.
    GameOver,
}

#[allow(dead_code)]
pub struct AsteroidsFieldGame {
    pub solver: AcousticSolver,
    pub gravity: GravityField,
    pub sonifier: BinauralCockpitSonifier,
    pub width: usize,
    pub height: usize,

    // Player Wave-Interceptor state
    pub ship_x: f32,
    pub ship_y: f32,
    pub ship_vx: f32,
    pub ship_vy: f32,
    pub ship_theta: f32,
    pub ship_omega: f32,
    pub ship_radius: f32,
    pub invulnerable_steps: u16,
    pub shield_visual_timer: u8,
    pub ship_thrusting_timer: u8,
    /// Hamiltonian Field Capacitor in `[0.0, 100.0]` charged by Antiphase Shield parries & wave harvesting.
    pub capacitor: f32,

    // Autopilot state
    pub autopilot_enabled: bool,
    pub autopilot_tick: u32,

    // Living Ecology of Actors
    pub asteroids: Vec<CohesiveBody>,
    pub aliens: Vec<AlienEnemy>,
    pub bolts: Vec<SolitonBolt>,
    pub next_body_id: u32,

    // Progression & Telemetry
    pub in_play: bool,
    pub lives: u32,
    pub score: u32,
    pub high_score: u32,
    pub wave_number: u32,
    pub topological_cleaves: u32,
    pub shield_parries: u32,
    pub alien_spawn_timer: u16,

    // Modes & Inspection
    pub wave_force_enabled: bool,
    pub view_mode: FieldViewMode,
    pub topology: WorldTopologyMode,
    pub field_energy: FieldEnergy,
    pub saved_branch: Option<AsteroidsBranchSnapshot>,
    pub gpu_accel: GpuFieldAccelerator,
    pub shared_audio_frame: std::sync::Arc<std::sync::Mutex<FieldAcousticFrame>>,
    pub last_audio_frame: FieldAcousticFrame,

    // RGBA8 pixel framebuffer
    pub pixel_buffer: Vec<u8>,
}

#[allow(dead_code)]
impl AsteroidsFieldGame {
    pub const VACUUM_GAMMA: f32 = 0.022;

    pub fn new(width: usize, height: usize) -> Self {
        let grid = GridConfig::new(width, height, 1.0, 1.0);
        let state = AcousticState::new(grid);

        // Reserve CFL headroom for fast PiezoQuartz (rho = 0.75, K = 1.65) and Obsidian (rho = 2.0, K = 2.05)
        let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, Self::VACUUM_GAMMA);
        material.density[0] = 0.70;
        material.bulk_modulus[0] = 2.10;
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        material.density[0] = 1.0;
        material.bulk_modulus[0] = 1.0;

        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::Periodic);
        let nl = NonlinearLaw {
            beta: 0.08,
            alpha: 1.20,
            threshold_decay: 0.065,
        };
        let config = SolverConfig::new(dt, 0.7, 4).with_nonlinearity(nl);
        let gpu_accel = GpuFieldAccelerator::new(&state, &material, &boundary, &config);
        let solver = AcousticSolver::new(state, material, boundary, config).unwrap();
        let gravity = GravityField::new(grid, 0.15);

        let mut game = Self {
            solver,
            gravity,
            sonifier: BinauralCockpitSonifier::new(2.5, 2048),
            width,
            height,
            ship_x: width as f32 * 0.5,
            ship_y: height as f32 * 0.5,
            ship_vx: 0.0,
            ship_vy: 0.0,
            ship_theta: -FRAC_PI_2,
            ship_omega: 0.0,
            ship_radius: 2.6,
            invulnerable_steps: 45,
            shield_visual_timer: 0,
            ship_thrusting_timer: 0,
            capacitor: 50.0,
            autopilot_enabled: false,
            autopilot_tick: 0,
            asteroids: Vec::new(),
            aliens: Vec::new(),
            bolts: Vec::new(),
            next_body_id: 1,
            in_play: true,
            lives: 3,
            score: 0,
            high_score: 0,
            wave_number: 1,
            topological_cleaves: 0,
            shield_parries: 0,
            alien_spawn_timer: 180,
            wave_force_enabled: true,
            view_mode: FieldViewMode::CompositeArcade,
            topology: WorldTopologyMode::ToroidalPeriodic,
            field_energy: FieldEnergy {
                potential: 0.0,
                kinetic: 0.0,
                total: 0.0,
            },
            saved_branch: None,
            gpu_accel,
            shared_audio_frame: std::sync::Arc::new(std::sync::Mutex::new(
                FieldAcousticFrame::default(),
            )),
            last_audio_frame: FieldAcousticFrame::default(),
            pixel_buffer: vec![0; width * height * 4],
        };

        game.spawn_wave(1);
        game.sync_all_bodies_to_lattice();
        game
    }

    #[inline]
    pub fn is_periodic(&self) -> bool {
        self.topology == WorldTopologyMode::ToroidalPeriodic
    }

    #[inline]
    pub fn periodic_dims(&self) -> Option<(f32, f32)> {
        if self.is_periodic() {
            Some((self.width as f32, self.height as f32))
        } else {
            None
        }
    }

    /// Toggle between Toroidal Periodic Arena and Gravity-Well Singularity Arena (`[M]`).
    pub fn toggle_topology_mode(&mut self) {
        self.topology = self.topology.next();
        self.gravity.wells.clear();

        match self.topology {
            WorldTopologyMode::ToroidalPeriodic => {
                self.solver.boundary.outer_rule = OuterBoundaryRule::Periodic;
            }
            WorldTopologyMode::GravityWellArena => {
                self.solver.boundary.outer_rule = OuterBoundaryRule::AbsorbingSponge;
                self.gravity.wells.push(GravityWell {
                    x: self.width as f32 * 0.5,
                    y: self.height as f32 * 0.5,
                    strength: 2.2,
                    core_radius: 6.5,
                });
            }
        }

        self.gpu_accel
            .sim
            .set_outer_rule(&self.gpu_accel.ctx, self.solver.boundary.outer_rule);
    }

    /// Spawn a new wave of mineralogical asteroids and alien antagonists.
    pub fn spawn_wave(&mut self, wave: u32) {
        self.asteroids.clear();
        self.aliens.clear();
        self.bolts.clear();

        let w = self.width as f32;
        let h = self.height as f32;
        let large_arena = self.width * self.height >= 24_000;
        let count = if large_arena {
            (12 + wave as usize * 2).min(18)
        } else {
            (5 + wave as usize).min(10)
        };

        let classes = [
            MineralClass::PiezoQuartz,
            MineralClass::ObsidianAnvil,
            MineralClass::BismuthCrystal,
        ];

        for i in 0..count {
            let angle = (i as f32 / count as f32) * TAU + 0.35 + (i % 2) as f32 * 0.14;
            let ring_scale = match i % 3 {
                0 => 0.29,
                1 => 0.37,
                _ => 0.44,
            };
            let dist = w.min(h) * ring_scale;
            let ax = (w * 0.5 + angle.cos() * dist).rem_euclid(w);
            let ay = (h * 0.5 + angle.sin() * dist).rem_euclid(h);

            let class = classes[i % classes.len()];
            let id = self.next_body_id;
            self.next_body_id += 1;

            // Multi-scale asteroid ecology: massive parent geodes, medium drifters, fast resonant shards
            let target_r = if large_arena {
                match i % 3 {
                    0 => 8.4,
                    1 => 6.6,
                    _ => 4.8,
                }
            } else {
                6.8
            };

            let mut rock = CohesiveBody::new_asteroid(
                id,
                ax,
                ay,
                target_r,
                6 + (i % 4),
                (i as u32 + 1) * 7919 + wave * 131,
                class,
            );
            let drift_ang = angle + 1.35 + (i as f32 * 0.45).sin() * 0.35;
            let speed = if target_r < 5.5 {
                0.24
            } else {
                0.14 + (i % 3) as f32 * 0.04
            };
            rock.vx = drift_ang.cos() * speed;
            rock.vy = drift_ang.sin() * speed;
            rock.omega = if i % 2 == 0 { 0.018 } else { -0.022 };
            self.asteroids.push(rock);
        }

        // Spawn alien antagonists: Bohmian Saucer + Harmonic Weaver on large arena or wave 2+
        self.spawn_alien(AlienKind::BohmianSaucer);
        if large_arena || wave >= 2 {
            self.spawn_alien(AlienKind::HarmonicWeaver);
        }
    }

    /// Spawn a field-coupled alien antagonist at the arena perimeter.
    pub fn spawn_alien(&mut self, kind: AlienKind) {
        let id = self.next_body_id;
        self.next_body_id += 1;
        let (x, y, mut hull) = match kind {
            AlienKind::BohmianSaucer => {
                let px = 16.0;
                let py = 16.0;
                (px, py, CohesiveBody::new_saucer(id, px, py, 5.2))
            }
            AlienKind::HarmonicWeaver => {
                let px = self.width as f32 - 16.0;
                let py = self.height as f32 - 16.0;
                (px, py, CohesiveBody::new_weaver_ship(id, px, py, 5.2))
            }
        };
        let _ = (x, y);
        hull.vx = 0.22;
        hull.vy = 0.12;
        hull.omega = match kind {
            AlienKind::BohmianSaucer => 0.032,
            AlienKind::HarmonicWeaver => 0.012,
        };

        self.aliens.push(AlienEnemy {
            kind,
            hull,
            fire_cooldown: 55,
            telegraph_timer: 0,
            aim_angle: 0.0,
        });
    }

    pub fn reset_game(&mut self) {
        self.solver.state.clear();
        self.gravity.clear();
        self.lives = 3;
        self.score = 0;
        self.wave_number = 1;
        self.topological_cleaves = 0;
        self.shield_parries = 0;
        self.capacitor = 50.0;
        self.in_play = true;

        self.ship_x = self.width as f32 * 0.5;
        self.ship_y = self.height as f32 * 0.5;
        self.ship_vx = 0.0;
        self.ship_vy = 0.0;
        self.ship_theta = -FRAC_PI_2;
        self.ship_omega = 0.0;
        self.invulnerable_steps = 45;

        self.spawn_wave(1);
        self.sync_all_bodies_to_lattice();
    }

    /// Rotate ship by torque input (`dir` in `[-1.0, 1.0]`).
    pub fn rotate_ship(&mut self, dir: f32) {
        self.ship_omega = (self.ship_omega + dir * 0.045).clamp(-0.14, 0.14);
        self.ship_theta = (self.ship_theta + self.ship_omega).rem_euclid(TAU);
    }

    /// Fire main reaction thruster: accelerates the ship forward and, when an asteroid or alien
    /// is within backblast range behind the nozzle, injects a directional exhaust plume that
    /// physically pushes nearby debris ("Exhaust Slap") without flooding empty space with ripples.
    pub fn thrust_forward(&mut self, power: f32) {
        let p = power.clamp(0.1, 1.5);
        self.ship_thrusting_timer = 4;
        let exhaust_angle = self.ship_theta + PI;
        let nozzle_x = self.ship_x + exhaust_angle.cos() * (self.ship_radius + 1.2);
        let nozzle_y = self.ship_y + exhaust_angle.sin() * (self.ship_radius + 1.2);

        // Excite soft radiative ion glow in the luminescence field behind the thruster nozzle
        let w = self.width as isize;
        let h = self.height as isize;
        if self.solver.state.luminescence.len() == self.width * self.height {
            let nx_i = nozzle_x.round() as isize;
            let ny_i = nozzle_y.round() as isize;
            for oy in -1..=1 {
                for ox in -1..=1 {
                    let gx = (nx_i + ox).rem_euclid(w) as usize;
                    let gy = (ny_i + oy).rem_euclid(h) as usize;
                    let idx = self.solver.state.config.index(gx, gy);
                    self.solver.state.luminescence[idx] =
                        (self.solver.state.luminescence[idx] + 0.55 * p).min(3.5);
                }
            }
        }

        let near_target = self.asteroids.iter().any(|r| {
            let (dx, dy) = self.delta_vec(nozzle_x, nozzle_y, r.x, r.y);
            dx * dx + dy * dy <= (r.radius + 12.0).powi(2)
        }) || self.aliens.iter().any(|a| {
            let (dx, dy) = self.delta_vec(nozzle_x, nozzle_y, a.hull.x, a.hull.y);
            dx * dx + dy * dy <= (a.hull.radius + 12.0).powi(2)
        });

        if self.wave_force_enabled && near_target {
            let periodic = self.is_periodic();
            let plume = ReactionThrusterPlume::new(nozzle_x, nozzle_y, 1.6, 1.15 * p, exhaust_angle);
            let (recoil_fx, recoil_fy) =
                plume.inject(&mut self.solver.state, &self.solver.material, periodic);
            self.ship_vx = (self.ship_vx + recoil_fx * 0.0085).clamp(-0.85, 0.85);
            self.ship_vy = (self.ship_vy + recoil_fy * 0.0085).clamp(-0.85, 0.85);
        } else {
            self.ship_vx = (self.ship_vx + self.ship_theta.cos() * 0.075 * p).clamp(-0.85, 0.85);
            self.ship_vy = (self.ship_vy + self.ship_theta.sin() * 0.075 * p).clamp(-0.85, 0.85);
        }
    }

    /// Engage Field-Brake / Phase Anchor (`[S]` / `Down`):
    /// Bleeds ship linear momentum into a forward-directed acoustic bow-shock wave packet,
    /// giving crisp arcade stopping power while blasting nearby shards out of your path!
    pub fn apply_field_brake(&mut self) {
        let speed = (self.ship_vx * self.ship_vx + self.ship_vy * self.ship_vy).sqrt();
        if speed > 0.05 && self.wave_force_enabled {
            let periodic = self.is_periodic();
            let move_angle = self.ship_vy.atan2(self.ship_vx);
            let bow_x = self.ship_x + move_angle.cos() * (self.ship_radius + 1.5);
            let bow_y = self.ship_y + move_angle.sin() * (self.ship_radius + 1.5);
            let mut bow_shock = DirectionalWavePacket::new(
                bow_x,
                bow_y,
                2.0,
                (speed * 2.4).clamp(0.6, 2.2),
                move_angle,
            );
            bow_shock.inject_wrapped(
                &mut self.solver.state,
                &self.solver.material,
                periodic,
            );
            if let Ok(mut guard) = self.shared_audio_frame.lock() {
                guard.trigger_shield_pulse(false);
            }
        }
        self.ship_vx *= 0.68;
        self.ship_vy *= 0.68;
        self.ship_omega *= 0.65;
    }

    /// Fire a coherent plasma-soliton bolt from the ship nose.
    /// If the ship is thrusting hard along its nose vector (Doppler Overcharge) or `overcharge_requested`
    /// is true with `capacitor >= 35`, fires an Overcharged Phase-Lance that cleaves Obsidian in 1 hit!
    pub fn fire_soliton_bolt(&mut self, overcharge_requested: bool) -> bool {
        let forward_speed =
            self.ship_vx * self.ship_theta.cos() + self.ship_vy * self.ship_theta.sin();
        let doppler_overcharged = forward_speed >= 0.34 && self.wave_force_enabled;
        let capacitor_overcharged = overcharge_requested && self.capacitor >= 35.0;

        if capacitor_overcharged {
            self.capacitor -= 35.0;
        }
        let overcharged = doppler_overcharged || capacitor_overcharged;

        let nose_x = self.ship_x + self.ship_theta.cos() * (self.ship_radius + 1.4);
        let nose_y = self.ship_y + self.ship_theta.sin() * (self.ship_radius + 1.4);
        let base_speed = if overcharged { 1.18 } else { 0.96 };
        let bvx = self.ship_theta.cos() * base_speed + self.ship_vx * 0.35;
        let bvy = self.ship_theta.sin() * base_speed + self.ship_vy * 0.35;

        if self.wave_force_enabled {
            let periodic = self.is_periodic();
            let amp = if overcharged { 2.6 } else { 1.35 };
            let mut packet =
                DirectionalWavePacket::new(nose_x, nose_y, 1.5, amp, self.ship_theta);
            packet.inject_wrapped(
                &mut self.solver.state,
                &self.solver.material,
                periodic,
            );
        }

        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_player_laser(overcharged);
        }

        self.bolts.push(SolitonBolt {
            x: nose_x,
            y: nose_y,
            vx: bvx,
            vy: bvy,
            radius: if overcharged { 1.8 } else { 1.3 },
            ttl: 95,
            from_player: true,
            overcharged,
        });
        true
    }

    /// Trigger the Antiphase Interference Shield (`[Shift]` / `Right-Click`):
    /// Emits an antiphase carrier (`delta_phi = pi`) that cancels incoming wave excitations and
    /// enemy soliton bolts within the shield ring, harvesting their Hamiltonian energy into `capacitor`.
    pub fn trigger_antiphase_shield(&mut self) -> f32 {
        self.shield_visual_timer = 10;
        let shield_r = self.ship_radius + 5.5;
        let periodic = self.is_periodic();
        let w = self.width as f32;
        let h = self.height as f32;

        // 1. Cancel local acoustic wave excitations and harvest Hamiltonian energy
        let pulse = AntiphaseShieldPulse::new(
            self.ship_x,
            self.ship_y,
            1.2,
            shield_r,
            0.88,
        );
        let mut harvested = pulse.cancel_and_harvest(
            &mut self.solver.state,
            &self.solver.material,
            periodic,
        );

        // 2. Annihilate any enemy soliton bolts inside the destructive interference ring
        let sx = self.ship_x;
        let sy = self.ship_y;
        let mut parried_bolts = 0u32;
        self.bolts.retain(|b| {
            if b.from_player {
                return true;
            }
            let mut dx = b.x - sx;
            let mut dy = b.y - sy;
            if periodic {
                if dx > 0.5 * w { dx -= w; } else if dx < -0.5 * w { dx += w; }
                if dy > 0.5 * h { dy -= h; } else if dy < -0.5 * h { dy += h; }
            }
            if dx * dx + dy * dy <= (shield_r + 1.5).powi(2) {
                parried_bolts += 1;
                false
            } else {
                true
            }
        });

        if parried_bolts > 0 {
            harvested += parried_bolts as f32 * 22.0;
            self.shield_parries += parried_bolts;
            self.score += parried_bolts * 125;
        }

        let gain = (harvested * 2.5).clamp(0.0, 50.0);
        self.capacitor = (self.capacitor + gain).min(100.0);
        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_shield_pulse(parried_bolts > 0);
        }
        harvested
    }

    pub fn cycle_view_mode(&mut self) {
        self.view_mode = self.view_mode.next();
    }

    pub fn toggle_wave_force(&mut self) {
        self.wave_force_enabled = !self.wave_force_enabled;
    }

    /// Save a complete situation branch checkpoint (`[B]`).
    pub fn save_branch_checkpoint(&mut self) {
        self.saved_branch = Some(AsteroidsBranchSnapshot {
            checkpoint: Checkpoint::capture(&self.solver),
            ship_x: self.ship_x,
            ship_y: self.ship_y,
            ship_vx: self.ship_vx,
            ship_vy: self.ship_vy,
            ship_theta: self.ship_theta,
            ship_omega: self.ship_omega,
            capacitor: self.capacitor,
            asteroids: self.asteroids.clone(),
            aliens: self.aliens.clone(),
            bolts: self.bolts.clone(),
            lives: self.lives,
            score: self.score,
            wave_number: self.wave_number,
            topology: self.topology,
        });
    }

    /// Restore a saved situation branch checkpoint (`[U]`).
    pub fn restore_branch_checkpoint(&mut self) -> bool {
        if let Some(branch) = &self.saved_branch {
            if let Ok(restored_solver) = branch.checkpoint.restore() {
                self.solver = restored_solver;
                self.ship_x = branch.ship_x;
                self.ship_y = branch.ship_y;
                self.ship_vx = branch.ship_vx;
                self.ship_vy = branch.ship_vy;
                self.ship_theta = branch.ship_theta;
                self.ship_omega = branch.ship_omega;
                self.capacitor = branch.capacitor;
                self.asteroids = branch.asteroids.clone();
                self.aliens = branch.aliens.clone();
                self.bolts = branch.bolts.clone();
                self.lives = branch.lives;
                self.score = branch.score;
                self.wave_number = branch.wave_number;
                self.topology = branch.topology;
                self.in_play = true;
                self.field_energy =
                    FieldEnergy::compute(&self.solver.state, &self.solver.material);
                return true;
            }
        }
        false
    }

    /// Synchronize all rotating asteroids, alien hulls, and the player ship into `MaterialField` & `BoundaryMask`.
    pub fn sync_all_bodies_to_lattice(&mut self) {
        let grid = self.solver.state.config;
        let periodic = self.is_periodic();

        // Reset interior material to clean quantum vacuum and clear walls
        self.solver.boundary.clear_interior();
        self.solver.material.density.fill(1.0);
        self.solver.material.bulk_modulus.fill(1.0);
        self.solver.material.damping.fill(Self::VACUUM_GAMMA);
        self.solver.material.cohesion.fill(0.0);
        self.solver.material.yield_pressure.fill(f32::INFINITY);

        if !periodic {
            self.solver.material.apply_sponge_layer(5, 0.25);
        }

        // 1. Rasterize rotating asteroids
        for rock in &self.asteroids {
            rock.rasterize_to_lattice(&mut self.solver.material, &mut self.solver.boundary, periodic);
        }

        // 2. Rasterize rotating alien hulls
        for alien in &self.aliens {
            alien
                .hull
                .rasterize_to_lattice(&mut self.solver.material, &mut self.solver.boundary, periodic);
        }

        // 3. Stamp player ship hull as a reflective boundary in clean vacuum
        // (zero imposed wall_vx/wall_vy so free drift in empty space never generates water-like wakes)
        let w = grid.width as isize;
        let h = grid.height as isize;
        let sx = self.ship_x.round() as isize;
        let sy = self.ship_y.round() as isize;

        for oy in -1..=1 {
            for ox in -1..=1 {
                let px = sx + ox;
                let py = sy + oy;
                if !periodic && (px < 1 || px >= w - 1 || py < 1 || py >= h - 1) {
                    continue;
                }
                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;
                let idx = grid.index(gx, gy);
                self.solver.boundary.wall_east[idx] = true;
                self.solver.boundary.wall_vx[idx] = 0.0;
                self.solver.boundary.wall_north[idx] = true;
                self.solver.boundary.wall_vy[idx] = 0.0;
            }
        }
    }

    /// Shortest delta vector `(dx, dy)` from `(x1, y1)` to `(x2, y2)`, respecting periodic wrap.
    #[inline]
    pub fn delta_vec(&self, x1: f32, y1: f32, x2: f32, y2: f32) -> (f32, f32) {
        let mut dx = x2 - x1;
        let mut dy = y2 - y1;
        if self.is_periodic() {
            let w = self.width as f32;
            let h = self.height as f32;
            if dx > 0.5 * w { dx -= w; } else if dx < -0.5 * w { dx += w; }
            if dy > 0.5 * h { dy -= h; } else if dy < -0.5 * h { dy += h; }
        }
        (dx, dy)
    }

    /// Wrap or clamp a coordinate pair `(x, y)` into the arena.
    #[inline]
    pub fn wrap_coords(&self, x: f32, y: f32) -> (f32, f32) {
        let w = self.width as f32;
        let h = self.height as f32;
        if self.is_periodic() {
            (x.rem_euclid(w), y.rem_euclid(h))
        } else {
            (x.clamp(4.0, w - 5.0), y.clamp(4.0, h - 5.0))
        }
    }

    /// Advance one macro-frame of Asteroids gameplay, rotating body dynamics, alien AI, and PDE substeps.
    pub fn step(&mut self) -> AsteroidEvent {
        if self.autopilot_enabled {
            self.step_autopilot();
        }

        if !self.in_play {
            return AsteroidEvent::None;
        }

        let periodic = self.is_periodic();
        let p_dims = self.periodic_dims();
        let sub_steps = 4;
        let dt_sub = 1.0 / sub_steps as f32;
        let mut event = AsteroidEvent::None;

        if self.invulnerable_steps > 0 {
            self.invulnerable_steps -= 1;
        }
        if self.shield_visual_timer > 0 {
            self.shield_visual_timer -= 1;
        }
        if self.ship_thrusting_timer > 0 {
            self.ship_thrusting_timer -= 1;
        }

        // Relax gravitational potential field Phi_g when GravityWellArena is active or heavy bodies exist
        if self.topology == WorldTopologyMode::GravityWellArena {
            self.gravity.relax_poisson(&self.solver.material, 4, periodic);
        }

        // 1. Integrate field forces (-grad p, -grad Q, -grad Phi_g) onto ship, asteroids, and aliens
        if self.wave_force_enabled {
            let sx = (self.ship_x.round() as isize).rem_euclid(self.width as isize) as usize;
            let sy = (self.ship_y.round() as isize).rem_euclid(self.height as isize) as usize;
            let (gpx, gpy) = self.solver.state.pressure_gradient_wrapped_at(sx, sy, periodic);
            if gpx * gpx + gpy * gpy > 0.06 {
                let (qfx, qfy) = self.solver.state.quantum_force_wrapped_at(sx, sy, periodic);
                self.ship_vx = (self.ship_vx + (-gpx * 0.025 + qfx * 0.004)).clamp(-0.85, 0.85);
                self.ship_vy = (self.ship_vy + (-gpy * 0.025 + qfy * 0.004)).clamp(-0.85, 0.85);
            }

            if self.topology == WorldTopologyMode::GravityWellArena {
                let (agx, agy) = self.gravity.acceleration_at(self.ship_x, self.ship_y, periodic);
                self.ship_vx = (self.ship_vx + agx * 0.25).clamp(-0.85, 0.85);
                self.ship_vy = (self.ship_vy + agy * 0.25).clamp(-0.85, 0.85);
            }

            // Zero-Readback GPU Force & Overpressure Reduction Pass
            let mut queries = Vec::with_capacity(self.asteroids.len());
            for rock in &self.asteroids {
                queries.push(GpuBodyQuery {
                    x: rock.x,
                    y: rock.y,
                    radius: rock.radius,
                    yield_pressure: rock.material.yield_pressure,
                });
            }
            let reductions = self
                .gpu_accel
                .sim
                .reduce_bodies_gpu(&self.gpu_accel.ctx, &queries);
            for (rock, red) in self.asteroids.iter_mut().zip(reductions.iter()) {
                rock.vx = (rock.vx + red.fx / rock.mass).clamp(-0.65, 0.65);
                rock.vy = (rock.vy + red.fy / rock.mass).clamp(-0.65, 0.65);
                rock.omega = (rock.omega + red.tau_z / rock.inertia).clamp(-0.14, 0.14);
                if red.max_pressure > rock.material.yield_pressure {
                    let excess = red.max_pressure - rock.material.yield_pressure;
                    for c in &mut rock.cohesion {
                        if *c > 0.0 {
                            *c = (*c - 0.12 * excess).max(0.0);
                        }
                    }
                }
                if self.topology == WorldTopologyMode::GravityWellArena {
                    let (agx, agy) = self.gravity.acceleration_at(rock.x, rock.y, periodic);
                    rock.vx = (rock.vx + agx * 0.20).clamp(-0.65, 0.65);
                    rock.vy = (rock.vy + agy * 0.20).clamp(-0.65, 0.65);
                }
            }
        }

        // 2. Step Alien Antagonists (Bohmian Pilot-Wave Navigation & Telegraphed Attacks)
        let mut new_alien_bolts = Vec::new();
        let ship_pos = (self.ship_x, self.ship_y);
        let first_rock_pos = self.asteroids.first().map(|r| (r.x, r.y, r.radius));

        for alien in &mut self.aliens {
            let ax = (alien.hull.x.round() as isize).rem_euclid(self.width as isize) as usize;
            let ay = (alien.hull.y.round() as isize).rem_euclid(self.height as isize) as usize;
            let (qfx, qfy) = self.solver.state.quantum_force_wrapped_at(ax, ay, periodic);
            let (gpx, gpy) = self.solver.state.pressure_gradient_wrapped_at(ax, ay, periodic);

            match alien.kind {
                AlienKind::BohmianSaucer => {
                    // Surf along Bohmian quantum potential minima (-grad Q) while stalking player at standoff range
                    let ( mut dx, mut dy) = (ship_pos.0 - alien.hull.x, ship_pos.1 - alien.hull.y);
                    if periodic {
                        let wf = self.width as f32;
                        let hf = self.height as f32;
                        if dx > 0.5 * wf { dx -= wf; } else if dx < -0.5 * wf { dx += wf; }
                        if dy > 0.5 * hf { dy -= hf; } else if dy < -0.5 * hf { dy += hf; }
                    }
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    let desired_dist = 32.0;
                    let radial_drive = ((dist - desired_dist) / desired_dist).clamp(-0.5, 0.5);
                    // Tangential orbit + Bohmian Q surfing + shockwave evasion (-grad p)
                    let tang_x = -dy / dist;
                    let tang_y = dx / dist;

                    let ax_drive = (dx / dist) * radial_drive * 0.03
                        + tang_x * 0.025
                        + qfx * 0.04
                        - gpx * 0.05;
                    let ay_drive = (dy / dist) * radial_drive * 0.03
                        + tang_y * 0.025
                        + qfy * 0.04
                        - gpy * 0.05;

                    alien.hull.vx = (alien.hull.vx * 0.96 + ax_drive).clamp(-0.38, 0.38);
                    alien.hull.vy = (alien.hull.vy * 0.96 + ay_drive).clamp(-0.38, 0.38);

                    // Telegraphed Soliton Attack
                    if alien.telegraph_timer > 0 {
                        alien.telegraph_timer -= 1;
                        if alien.telegraph_timer == 0 {
                            let aim = alien.aim_angle;
                            let bx = alien.hull.x + aim.cos() * (alien.hull.radius + 1.5);
                            let by = alien.hull.y + aim.sin() * (alien.hull.radius + 1.5);
                            if self.wave_force_enabled {
                                let mut pkt =
                                    DirectionalWavePacket::new(bx, by, 1.5, 1.4, aim);
                                pkt.inject_wrapped(
                                    &mut self.solver.state,
                                    &self.solver.material,
                                    periodic,
                                );
                            }
                            new_alien_bolts.push(SolitonBolt {
                                x: bx,
                                y: by,
                                vx: aim.cos() * 0.72,
                                vy: aim.sin() * 0.72,
                                radius: 1.4,
                                ttl: 90,
                                from_player: false,
                                overcharged: false,
                            });
                            if let Ok(mut guard) = self.shared_audio_frame.lock() {
                                let pan = FieldAcousticFrame::compute_stereo_pan(
                                    ship_pos.0,
                                    ship_pos.1,
                                    self.ship_theta,
                                    bx,
                                    by,
                                    p_dims,
                                );
                                guard.trigger_enemy_laser(pan);
                            }
                            alien.fire_cooldown = 70;
                        }
                    } else if alien.fire_cooldown > 0 {
                        alien.fire_cooldown -= 1;
                    } else {
                        // Lock onto player (visual telegraph timer before firing)
                        alien.aim_angle = dy.atan2(dx);
                        alien.telegraph_timer = 16;
                    }
                }
                AlienKind::HarmonicWeaver => {
                    // Orbit nearest asteroid and pump standing-wave acoustic stress into it
                    if let Some((rx, ry, r_rad)) = first_rock_pos {
                        let dx = rx - alien.hull.x;
                        let dy = ry - alien.hull.y;
                        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                        let aim = dy.atan2(dx);
                        alien.aim_angle = aim;
                        alien.hull.theta = aim;
                        let orbit_r = r_rad + 12.0;
                        let radial = ((dist - orbit_r) / orbit_r).clamp(-0.5, 0.5);
                        alien.hull.vx =
                            (alien.hull.vx * 0.95 + (dx / dist) * radial * 0.03 - (dy / dist) * 0.025)
                                .clamp(-0.34, 0.34);
                        alien.hull.vy =
                            (alien.hull.vy * 0.95 + (dy / dist) * radial * 0.03 + (dx / dist) * 0.025)
                                .clamp(-0.34, 0.34);

                        if alien.fire_cooldown > 0 {
                            alien.fire_cooldown -= 1;
                        } else {
                            alien.telegraph_timer = 12;
                            if self.wave_force_enabled {
                                let mut beam = DirectionalWavePacket::new(
                                    alien.hull.x + aim.cos() * 4.0,
                                    alien.hull.y + aim.sin() * 4.0,
                                    1.8,
                                    1.6,
                                    aim,
                                );
                                beam.inject_wrapped(
                                    &mut self.solver.state,
                                    &mut self.solver.material,
                                    periodic,
                                );
                            }
                            if let Ok(mut guard) = self.shared_audio_frame.lock() {
                                let pan = FieldAcousticFrame::compute_stereo_pan(
                                    ship_pos.0,
                                    ship_pos.1,
                                    self.ship_theta,
                                    alien.hull.x,
                                    alien.hull.y,
                                    p_dims,
                                );
                                guard.trigger_enemy_laser(pan);
                            }
                            alien.fire_cooldown = 45;
                        }
                        if alien.telegraph_timer > 0 {
                            alien.telegraph_timer -= 1;
                        }
                    }
                }
            }
        }
        self.bolts.extend(new_alien_bolts);

        // 3. Substep kinematics & collision resolution
        for _ in 0..sub_steps {
            // Advance ship
            let (nx, ny) = self.wrap_coords(
                self.ship_x + self.ship_vx * dt_sub,
                self.ship_y + self.ship_vy * dt_sub,
            );
            self.ship_x = nx;
            self.ship_y = ny;

            // Advance asteroids
            for rock in &mut self.asteroids {
                let (rx, ry) = if periodic {
                    (
                        (rock.x + rock.vx * dt_sub).rem_euclid(self.width as f32),
                        (rock.y + rock.vy * dt_sub).rem_euclid(self.height as f32),
                    )
                } else {
                    let mut x = rock.x + rock.vx * dt_sub;
                    let mut y = rock.y + rock.vy * dt_sub;
                    let w = self.width as f32;
                    let h = self.height as f32;
                    if x < rock.radius + 3.0 {
                        x = rock.radius + 3.0;
                        rock.vx = rock.vx.abs();
                    } else if x > w - rock.radius - 3.0 {
                        x = w - rock.radius - 3.0;
                        rock.vx = -rock.vx.abs();
                    }
                    if y < rock.radius + 3.0 {
                        y = rock.radius + 3.0;
                        rock.vy = rock.vy.abs();
                    } else if y > h - rock.radius - 3.0 {
                        y = h - rock.radius - 3.0;
                        rock.vy = -rock.vy.abs();
                    }
                    (x, y)
                };
                rock.x = rx;
                rock.y = ry;
                rock.theta = (rock.theta + rock.omega * dt_sub).rem_euclid(TAU);
            }

            // Advance aliens
            for alien in &mut self.aliens {
                alien.hull.x = (alien.hull.x + alien.hull.vx * dt_sub).rem_euclid(self.width as f32);
                alien.hull.y = (alien.hull.y + alien.hull.vy * dt_sub).rem_euclid(self.height as f32);
                alien.hull.theta = (alien.hull.theta + alien.hull.omega * dt_sub).rem_euclid(TAU);
            }

            // Advance soliton bolts and test impacts against asteroids, aliens, and player
            let old_bolts = std::mem::take(&mut self.bolts);
            let mut bolts_to_keep = Vec::with_capacity(old_bolts.len());
            let mut pending_rock_hits: Vec<(usize, f32, f32, f32, f32, bool)> = Vec::new();
            let mut pending_alien_hits: Vec<(usize, f32, f32, f32, f32, bool)> = Vec::new();
            let mut player_hit_by_bolt = false;

            for mut bolt in old_bolts {
                if bolt.ttl == 0 {
                    continue;
                }
                bolt.x += bolt.vx * dt_sub;
                bolt.y += bolt.vy * dt_sub;
                if periodic {
                    bolt.x = bolt.x.rem_euclid(self.width as f32);
                    bolt.y = bolt.y.rem_euclid(self.height as f32);
                } else if bolt.x < 2.0
                    || bolt.x >= self.width as f32 - 2.0
                    || bolt.y < 2.0
                    || bolt.y >= self.height as f32 - 2.0
                {
                    continue;
                }

                let mut consumed = false;

                // Check bolt vs asteroids
                for (r_idx, rock) in self.asteroids.iter().enumerate() {
                    let (dx, dy) = self.delta_vec(rock.x, rock.y, bolt.x, bolt.y);
                    if dx * dx + dy * dy <= (rock.radius + bolt.radius).powi(2) {
                        let (lx, ly) = rock.world_to_local(bolt.x, bolt.y, p_dims);
                        if rock.sample_local_cohesion(lx, ly) >= 0.15
                            || dx * dx + dy * dy <= (rock.radius * 0.78).powi(2)
                        {
                            pending_rock_hits.push((
                                r_idx,
                                bolt.x,
                                bolt.y,
                                bolt.vx,
                                bolt.vy,
                                bolt.overcharged,
                            ));
                            consumed = true;
                            break;
                        }
                    }
                }

                // Check player bolt vs alien enemies
                if !consumed && bolt.from_player {
                    for (a_idx, alien) in self.aliens.iter().enumerate() {
                        let (dx, dy) = self.delta_vec(alien.hull.x, alien.hull.y, bolt.x, bolt.y);
                        if dx * dx + dy * dy <= (alien.hull.radius + bolt.radius).powi(2) {
                            pending_alien_hits.push((
                                a_idx,
                                bolt.x,
                                bolt.y,
                                bolt.vx,
                                bolt.vy,
                                bolt.overcharged,
                            ));
                            consumed = true;
                            break;
                        }
                    }
                }

                // Check alien bolt vs player ship
                if !consumed && !bolt.from_player && self.invulnerable_steps == 0 {
                    let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, bolt.x, bolt.y);
                    if dx * dx + dy * dy <= (self.ship_radius + bolt.radius).powi(2) {
                        player_hit_by_bolt = true;
                        consumed = true;
                    }
                }

                if !consumed {
                    if self.wave_force_enabled {
                        let (c_amp, l_gain, c_sigma) = if bolt.overcharged {
                            (1.35, 0.95, 1.55)
                        } else if bolt.from_player {
                            (0.82, 0.65, 1.20)
                        } else {
                            (0.78, 0.60, 1.20)
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
                        .couple_to_lattice(&mut self.solver.state, &self.solver.material, periodic);
                    }
                    bolts_to_keep.push(bolt);
                }
            }
            self.bolts = bolts_to_keep;

            // Process asteroid impacts (Impedance reflection, Torque, Cleavage, and BFS Topological Splitting)
            if !pending_rock_hits.is_empty() {
                pending_rock_hits.sort_by(|a, b| b.0.cmp(&a.0));
                pending_rock_hits.dedup_by_key(|h| h.0);

                for (r_idx, hx, hy, bvx, bvy, overcharged) in pending_rock_hits {
                    if r_idx >= self.asteroids.len() {
                        continue;
                    }
                    let b_speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                    let dir_x = bvx / b_speed;
                    let dir_y = bvy / b_speed;
                    let energy = if overcharged { 2.2 } else { 1.1 };

                    // Impart linear & angular recoil momentum (r x J)
                    self.asteroids[r_idx].apply_contact_impulse(
                        hx,
                        hy,
                        dir_x * 1.4 * energy,
                        dir_y * 1.4 * energy,
                        p_dims,
                    );

                    let cleaved = self.asteroids[r_idx].carve_cleavage_fissure(
                        hx, hy, dir_x, dir_y, energy, p_dims,
                    );
                    let parent_id = self.asteroids[r_idx].id;
                    let class = self.asteroids[r_idx].material.class;
                    let mat_c = (self.asteroids[r_idx].material.bulk_modulus
                        / self.asteroids[r_idx].material.rho)
                        .sqrt();
                    let hit_pan = FieldAcousticFrame::compute_stereo_pan(
                        self.ship_x,
                        self.ship_y,
                        self.ship_theta,
                        hx,
                        hy,
                        p_dims,
                    );
                    let active_cells = self.asteroids[r_idx].active_cell_count();

                    if active_cells < 14 {
                        // Small shard vaporizes into a cavitation shockwave!
                        let removed = self.asteroids.swap_remove(r_idx);
                        self.score += 200;
                        self.capacitor = (self.capacitor + 10.0).min(100.0);
                        if self.wave_force_enabled {
                            let blast = GaussianImpulse::new(removed.x, removed.y, 2.8, 3.8);
                            blast.inject_wrapped(&mut self.solver.state, periodic);
                        }
                        if let Ok(mut guard) = self.shared_audio_frame.lock() {
                            guard.trigger_cavitation_break(hit_pan, 1.55);
                        }
                        event = AsteroidEvent::AsteroidShattered {
                            body_id: parent_id,
                            remaining: self.asteroids.len(),
                        };
                    } else if cleaved {
                        let children = self.asteroids[r_idx]
                            .split_connected_components(&mut self.next_body_id, p_dims);
                        if children.len() >= 2 {
                            let removed = self.asteroids.swap_remove(r_idx);
                            let num_children = children.len();
                            self.asteroids.extend(children);
                            self.topological_cleaves += 1;
                            self.score += 150 * num_children as u32;
                            self.capacitor = (self.capacitor + 14.0).min(100.0);

                            if self.wave_force_enabled {
                                // PiezoQuartz emits an extra-strong piezo shock ring + 3 crystal-axis phonon bursts!
                                let amp = if class == MineralClass::PiezoQuartz { 4.2 } else { 3.0 };
                                let blast = GaussianImpulse::new(removed.x, removed.y, 2.6, amp);
                                blast.inject_wrapped(&mut self.solver.state, periodic);

                                if class == MineralClass::PiezoQuartz {
                                    for k_axis in 0..3 {
                                        let axis_ang = removed.theta + (k_axis as f32) * (TAU / 3.0);
                                        let mut piezo_ray = DirectionalWavePacket::new(
                                            removed.x + axis_ang.cos() * 2.5,
                                            removed.y + axis_ang.sin() * 2.5,
                                            1.6,
                                            2.1,
                                            axis_ang,
                                        );
                                        piezo_ray.inject_wrapped(
                                            &mut self.solver.state,
                                            &self.solver.material,
                                            periodic,
                                        );
                                    }
                                }
                            }
                            if let Ok(mut guard) = self.shared_audio_frame.lock() {
                                guard.trigger_cavitation_break(hit_pan, 1.45);
                            }
                            event = AsteroidEvent::AsteroidCleaved {
                                parent_id,
                                children_spawned: num_children,
                            };
                        } else {
                            self.score += 60;
                            if self.wave_force_enabled {
                                let pulse = GaussianImpulse::new(hx, hy, 1.6, 1.2);
                                pulse.inject_wrapped(&mut self.solver.state, periodic);
                            }
                            if let Ok(mut guard) = self.shared_audio_frame.lock() {
                                guard.trigger_crystal_crack(hit_pan, mat_c);
                            }
                            event = AsteroidEvent::AsteroidCracked {
                                body_id: parent_id,
                                class,
                            };
                        }
                    } else {
                        // First strike carved an internal waveguide fissure & imparted spin
                        self.score += 50;
                        if self.wave_force_enabled {
                            let pulse = GaussianImpulse::new(hx, hy, 1.5, 1.0);
                            pulse.inject_wrapped(&mut self.solver.state, periodic);
                        }
                        if let Ok(mut guard) = self.shared_audio_frame.lock() {
                            guard.trigger_crystal_crack(hit_pan, mat_c);
                        }
                        event = AsteroidEvent::AsteroidCracked {
                            body_id: parent_id,
                            class,
                        };
                    }
                }
            }

            // Process alien enemy hits
            if !pending_alien_hits.is_empty() {
                pending_alien_hits.sort_by(|a, b| b.0.cmp(&a.0));
                pending_alien_hits.dedup_by_key(|h| h.0);

                for (a_idx, hx, hy, bvx, bvy, overcharged) in pending_alien_hits {
                    if a_idx >= self.aliens.len() {
                        continue;
                    }
                    let b_speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                    let dir_x = bvx / b_speed;
                    let dir_y = bvy / b_speed;
                    let energy = if overcharged { 2.4 } else { 1.85 };
                    let hit_pan = FieldAcousticFrame::compute_stereo_pan(
                        self.ship_x,
                        self.ship_y,
                        self.ship_theta,
                        hx,
                        hy,
                        p_dims,
                    );

                    let cleaved = self.aliens[a_idx].hull.carve_cleavage_fissure(
                        hx, hy, dir_x, dir_y, energy, p_dims,
                    );
                    if cleaved || self.aliens[a_idx].hull.mean_cohesion() < 0.72 {
                        let destroyed = self.aliens.swap_remove(a_idx);
                        // Cleave alien hull into tumbling crystal debris shards!
                        let shards = destroyed
                            .hull
                            .split_connected_components(&mut self.next_body_id, p_dims);
                        if shards.len() >= 2 {
                            self.topological_cleaves += 1;
                            self.asteroids.extend(shards);
                        }
                        self.score += 500;
                        self.capacitor = (self.capacitor + 25.0).min(100.0);
                        if self.wave_force_enabled {
                            let blast = GaussianImpulse::new(
                                destroyed.hull.x,
                                destroyed.hull.y,
                                3.2,
                                4.5,
                            );
                            blast.inject_wrapped(&mut self.solver.state, periodic);
                        }
                        if let Ok(mut guard) = self.shared_audio_frame.lock() {
                            guard.trigger_cavitation_break(hit_pan, 1.75);
                        }
                        event = AsteroidEvent::AlienDestroyed {
                            kind: destroyed.kind,
                        };
                    } else if let Ok(mut guard) = self.shared_audio_frame.lock() {
                        guard.trigger_crystal_crack(hit_pan, 1.25);
                    }
                }
            }

            // Asteroid-to-Asteroid elastic + acoustic collision
            let n_rocks = self.asteroids.len();
            for i in 0..n_rocks {
                for j in (i + 1)..n_rocks {
                    let (dx, dy) = self.delta_vec(
                        self.asteroids[i].x,
                        self.asteroids[i].y,
                        self.asteroids[j].x,
                        self.asteroids[j].y,
                    );
                    let min_dist = (self.asteroids[i].radius + self.asteroids[j].radius) * 0.82;
                    let d2 = dx * dx + dy * dy;
                    if d2 < min_dist * min_dist && d2 > 0.01 {
                        let d = d2.sqrt();
                        let nx = dx / d;
                        let ny = dy / d;
                        let overlap = min_dist - d;
                        self.asteroids[i].x =
                            (self.asteroids[i].x - nx * overlap * 0.5).rem_euclid(self.width as f32);
                        self.asteroids[i].y =
                            (self.asteroids[i].y - ny * overlap * 0.5).rem_euclid(self.height as f32);
                        self.asteroids[j].x =
                            (self.asteroids[j].x + nx * overlap * 0.5).rem_euclid(self.width as f32);
                        self.asteroids[j].y =
                            (self.asteroids[j].y + ny * overlap * 0.5).rem_euclid(self.height as f32);

                        let rvx = self.asteroids[j].vx - self.asteroids[i].vx;
                        let rvy = self.asteroids[j].vy - self.asteroids[i].vy;
                        let vn = rvx * nx + rvy * ny;
                        if vn < 0.0 {
                            let imp = -1.5 * vn;
                            self.asteroids[i].vx -= nx * imp * 0.5;
                            self.asteroids[i].vy -= ny * imp * 0.5;
                            self.asteroids[j].vx += nx * imp * 0.5;
                            self.asteroids[j].vy += ny * imp * 0.5;
                        }
                    }
                }
            }

            // Check ship collision with asteroids or alien bolts
            let mut ship_struck = player_hit_by_bolt;
            if !ship_struck && self.invulnerable_steps == 0 {
                for rock in &self.asteroids {
                    let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, rock.x, rock.y);
                    if dx * dx + dy * dy <= (self.ship_radius + rock.radius * 0.78).powi(2) {
                        ship_struck = true;
                        break;
                    }
                }
            }

            if ship_struck {
                self.lives = self.lives.saturating_sub(1);
                self.invulnerable_steps = 60;
                self.ship_vx *= -0.5;
                self.ship_vy *= -0.5;
                if self.wave_force_enabled {
                    let blast = GaussianImpulse::new(self.ship_x, self.ship_y, 2.8, 3.2);
                    blast.inject_wrapped(&mut self.solver.state, periodic);
                }
                if let Ok(mut guard) = self.shared_audio_frame.lock() {
                    guard.trigger_cavitation_break(0.0, 1.65);
                }
                if self.lives == 0 && !self.autopilot_enabled {
                    self.in_play = false;
                    event = AsteroidEvent::GameOver;
                    break;
                } else {
                    if self.lives == 0 && self.autopilot_enabled {
                        self.lives = 3;
                    }
                    event = AsteroidEvent::ShipDamaged {
                        remaining_lives: self.lives,
                    };
                }
            }
        }

        // Decrement bolt TTLs
        for bolt in &mut self.bolts {
            bolt.ttl = bolt.ttl.saturating_sub(1);
        }

        // Gentle rotational damping on player ship so steering feels crisp
        self.ship_omega *= 0.82;
        self.ship_vx *= 0.992;
        self.ship_vy *= 0.992;

        // 4. Synchronize all rotating bodies into the lattice and advance WGPU FDTD substeps
        self.sync_all_bodies_to_lattice();

        let f_substeps = self.solver.config.substeps_per_macro;
        let gpu = &self.gpu_accel;
        gpu.sim.upload_state(&gpu.ctx, &self.solver.state);
        gpu.sim.upload_material(&gpu.ctx, &self.solver.material);
        gpu.sim.upload_boundary(&gpu.ctx, &self.solver.boundary);
        gpu.sim.step(&gpu.ctx, f_substeps);
        gpu.sim.readback_state_into(&gpu.ctx, &mut self.solver.state);
        self.solver.current_step += f_substeps;
        self.solver.current_time += self.solver.config.dt * f_substeps as f32;

        // 5. Sympathetic Wave Cleaving (distant cavitation shockwaves cleaving already-cracked rocks)
        if self.wave_force_enabled && self.in_play {
            self.check_sympathetic_wave_cleaving(&mut event);
        }

        // 6. Live Binaural Cockpit & Lattice Modal Sonification
        self.last_audio_frame = self.sonifier.sample_full_field(
            &self.solver.state,
            &self.solver.material,
            self.ship_x,
            self.ship_y,
            self.ship_vx,
            self.ship_vy,
            self.ship_theta,
            periodic,
        );
        if let Ok(mut guard) = self.shared_audio_frame.try_lock() {
            guard.merge_telemetry(&self.last_audio_frame);
        }

        // 7. Wave progression
        if self.in_play && self.asteroids.is_empty() && self.aliens.is_empty() {
            self.wave_number += 1;
            let next_w = self.wave_number;
            self.spawn_wave(next_w);
            event = AsteroidEvent::WaveCleared { next_wave: next_w };
        }

        self.field_energy = FieldEnergy::compute(&self.solver.state, &self.solver.material);
        if self.score > self.high_score {
            self.high_score = self.score;
        }

        event
    }

    /// Check if external shockwaves have eroded an already-cracked asteroid past topological cleavage.
    fn check_sympathetic_wave_cleaving(&mut self, event: &mut AsteroidEvent) {
        let p_dims = self.periodic_dims();
        let periodic = self.is_periodic();
        let mut i = 0;
        while i < self.asteroids.len() {
            if self.asteroids[i].mean_cohesion() < 0.68 {
                let children =
                    self.asteroids[i].split_connected_components(&mut self.next_body_id, p_dims);
                if children.len() >= 2 {
                    let removed = self.asteroids.swap_remove(i);
                    let num_c = children.len();
                    self.asteroids.extend(children);
                    self.topological_cleaves += 1;
                    self.score += 200 * num_c as u32;
                    if self.wave_force_enabled {
                        let blast = GaussianImpulse::new(removed.x, removed.y, 2.4, 2.8);
                        blast.inject_wrapped(&mut self.solver.state, periodic);
                    }
                    if let Ok(mut guard) = self.shared_audio_frame.lock() {
                        let pan = FieldAcousticFrame::compute_stereo_pan(
                            self.ship_x,
                            self.ship_y,
                            self.ship_theta,
                            removed.x,
                            removed.y,
                            p_dims,
                        );
                        guard.trigger_cavitation_break(pan, 1.45);
                    }
                    if *event == AsteroidEvent::None {
                        *event = AsteroidEvent::AsteroidCleaved {
                            parent_id: removed.id,
                            children_spawned: num_c,
                        };
                    }
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Render the current field & actor ecology into `self.pixel_buffer` (`RGBA8`).
    pub fn render_to_pixel_buffer(&mut self) {
        let w = self.width;
        let h = self.height;
        let periodic = self.is_periodic();
        let p_dims = self.periodic_dims();

        if self.pixel_buffer.len() != w * h * 4 {
            self.pixel_buffer.resize(w * h * 4, 0);
        }

        let has_lum = self.solver.state.luminescence.len() == w * h;

        // 1. Background field & Radiative Luminescence L(x, y) projection
        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let p = self.solver.state.pressure[idx];
                let vx = self.solver.state.vx[idx];
                let vy = self.solver.state.vy[idx];
                let lum = if has_lum { self.solver.state.luminescence[idx] } else { 0.0 };
                let rho = self.solver.material.density[idx];
                let k = self.solver.material.bulk_modulus[idx];
                let coh = self.solver.material.cohesion[idx];

                // Deterministic deep-space starfield shimmer in quiet vacuum
                let star_hash = (x as u32)
                    .wrapping_mul(374761393)
                    .wrapping_add((y as u32).wrapping_mul(668265263));
                let star = if (star_hash & 0x1FF) == 0 {
                    0.18 + ((star_hash >> 9) & 0x7) as f32 * 0.03
                } else {
                    0.0
                };

                let (mut r, mut g, mut b): (f32, f32, f32) = match self.view_mode {
                    FieldViewMode::CompositeArcade => {
                        let p_pos = p.max(0.0);
                        let p_neg = (-p).max(0.0);
                        let v_mag = (vx * vx + vy * vy).sqrt();
                        // Field-driven exciton luminescence glow (plasma cyan / solar gold)
                        let l_glow = lum.min(3.0);
                        (
                            0.015 + star * 0.8 + p_pos * 0.42 + v_mag * 0.14 + l_glow * 0.24,
                            0.022 + star * 0.9 + p_pos * 0.26 + p_neg * 0.34 + v_mag * 0.24 + l_glow * 0.62,
                            0.048 + star + p_neg * 0.62 + p_pos * 0.22 + l_glow * 0.82,
                        )
                    }
                    FieldViewMode::PressureAndBohmian => {
                        let q = self
                            .solver
                            .state
                            .quantum_potential_wrapped_at(x, y, periodic);
                        let p_pos = p.max(0.0);
                        let p_neg = (-p).max(0.0);
                        let q_well = (-q).max(0.0) * 0.08;
                        (
                            0.03 + p_pos * 0.65 + q_well * 0.55 + lum * 0.20,
                            0.03 + q_well * 0.85 + lum * 0.45,
                            0.06 + p_neg * 0.70 + q_well * 0.40 + lum * 0.55,
                        )
                    }
                    FieldViewMode::VelocityAndFlux => {
                        let v_mag = (vx * vx + vy * vy).sqrt();
                        let flux = (p * vx).hypot(p * vy);
                        let phi_g = (-self.gravity.potential[idx]).max(0.0);
                        (
                            0.03 + flux * 0.65 + phi_g * 0.35 + lum * 0.30,
                            0.04 + v_mag * 0.75 + lum * 0.50,
                            0.07 + v_mag * 0.45 + phi_g * 0.75 + lum * 0.40,
                        )
                    }
                    FieldViewMode::ImpedanceAndCohesion => {
                        let z = (rho * k).sqrt();
                        let z_dev = (z - 1.0).abs();
                        (
                            0.04 + z_dev * 0.70,
                            0.04 + coh * 0.65,
                            0.07 + (1.0 - coh) * z_dev * 0.85,
                        )
                    }
                };

                if self.view_mode == FieldViewMode::ImpedanceAndCohesion && coh >= 0.15 {
                    r += 0.25 * (1.0 - coh);
                    g += 0.35 * coh;
                    b += 0.30;
                }

                let p_idx = (y * w + x) * 4;
                self.pixel_buffer[p_idx] = (r.clamp(0.0, 1.0) * 255.0) as u8;
                self.pixel_buffer[p_idx + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
                self.pixel_buffer[p_idx + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
                self.pixel_buffer[p_idx + 3] = 255;
            }
        }

        // 2. Render Faceted 3D Mineralogical Asteroids with Dynamic Field Illumination
        for rock in &self.asteroids {
            let r_bound = (rock.radius.ceil() as isize) + 1;
            let cx = rock.x.round() as isize;
            let cy = rock.y.round() as isize;

            for oy in -r_bound..=r_bound {
                for ox in -r_bound..=r_bound {
                    let px = cx + ox;
                    let py = cy + oy;
                    if !periodic && (px < 0 || px >= w as isize || py < 0 || py >= h as isize) {
                        continue;
                    }
                    let gx = px.rem_euclid(w as isize) as usize;
                    let gy = py.rem_euclid(h as isize) as usize;

                    let (lx, ly) = rock.world_to_local(px as f32, py as f32, p_dims);
                    let coh = rock.sample_local_cohesion(lx, ly);
                    if coh < 0.12 {
                        continue;
                    }

                    let idx = self.solver.state.config.index(gx, gy);
                    let p_val = self.solver.state.pressure[idx];
                    let lum_val = if has_lum { self.solver.state.luminescence[idx] } else { 0.0 };
                    let (glx, gly) = self
                        .solver
                        .state
                        .luminescence_gradient_wrapped_at(gx, gy, periodic);
                    let (gpx, gpy) = self
                        .solver
                        .state
                        .pressure_gradient_wrapped_at(gx, gy, periodic);

                    let (elev, w_nx, w_ny, ridge) = rock.sample_crystal_facet(lx, ly);

                    // Zenith starlight (-0.45, -0.65) + dynamic field illumination from nearby solitons/blasts
                    let zenith_dot = (-0.45 * w_nx - 0.65 * w_ny + 0.60 * elev).clamp(0.18, 1.25);
                    let field_light_x = glx * 1.4 + gpx.abs() * 0.35;
                    let field_light_y = gly * 1.4 + gpy.abs() * 0.35;
                    let field_dot = (w_nx * field_light_x + w_ny * field_light_y).max(0.0)
                        + lum_val * 0.42
                        + p_val.abs().min(2.0) * 0.22;

                    let (r, g, b) = if coh < 0.55 {
                        // Glowing semi-permeable internal cleavage waveguide vein!
                        let vein_pulse = (p_val.abs() + lum_val).min(2.5) * 0.35;
                        match rock.material.class {
                            MineralClass::PiezoQuartz => (
                                0.55 + vein_pulse * 0.45,
                                0.96,
                                1.0,
                            ),
                            MineralClass::ObsidianAnvil => (
                                1.0,
                                0.52 + vein_pulse * 0.40,
                                0.18 + vein_pulse * 0.25,
                            ),
                            _ => (
                                0.98,
                                0.88,
                                0.38 + vein_pulse * 0.45,
                            ),
                        }
                    } else {
                        match rock.material.class {
                            MineralClass::PiezoQuartz => {
                                // Translucent cyan-amethyst hexagonal crystal with bright prism ridges
                                let base_r = 0.18 * zenith_dot + ridge * 0.28 + field_dot * 0.38;
                                let base_g = 0.62 * zenith_dot + ridge * 0.34 + field_dot * 0.52;
                                let base_b = 0.88 * zenith_dot + ridge * 0.26 + field_dot * 0.45;
                                (base_r, base_g, base_b)
                            }
                            MineralClass::ObsidianAnvil => {
                                // Dark volcanic basalt-obsidian with razor-sharp crimson-silver specular bevels
                                let spec = (ridge * 0.52 + field_dot * 0.55).min(0.95);
                                let base_r = 0.24 * zenith_dot + spec * 0.72;
                                let base_g = 0.14 * zenith_dot + spec * 0.30;
                                let base_b = 0.22 * zenith_dot + spec * 0.44;
                                (base_r, base_g, base_b)
                            }
                            MineralClass::BismuthCrystal => {
                                // Stepped hopper-crystal terraces with thin-film amber/teal/magenta iridescence
                                let irid_phase = (lx * 0.55 + ly * 0.45 + p_val * 0.8).sin() * 0.5 + 0.5;
                                let base_r = (0.68 - 0.28 * irid_phase) * zenith_dot + ridge * 0.32 + field_dot * 0.40;
                                let base_g = (0.42 + 0.32 * irid_phase) * zenith_dot + ridge * 0.28 + field_dot * 0.42;
                                let base_b = (0.24 + 0.48 * (1.0 - irid_phase)) * zenith_dot + ridge * 0.35 + field_dot * 0.45;
                                (base_r, base_g, base_b)
                            }
                            MineralClass::AlienResonator => (
                                0.20 * zenith_dot + ridge * 0.3,
                                0.82 * zenith_dot + ridge * 0.2,
                                0.52 * zenith_dot + ridge * 0.3,
                            ),
                        }
                    };

                    // Smooth edge blending at crystal silhouette boundary
                    let alpha = ((coh - 0.12) / 0.30).clamp(0.25, 1.0);
                    let p_idx = (gy * w + gx) * 4;
                    let bg_r = self.pixel_buffer[p_idx] as f32 / 255.0;
                    let bg_g = self.pixel_buffer[p_idx + 1] as f32 / 255.0;
                    let bg_b = self.pixel_buffer[p_idx + 2] as f32 / 255.0;

                    let out_r = bg_r * (1.0 - alpha) + r.clamp(0.0, 1.0) * alpha;
                    let out_g = bg_g * (1.0 - alpha) + g.clamp(0.0, 1.0) * alpha;
                    let out_b = bg_b * (1.0 - alpha) + b.clamp(0.0, 1.0) * alpha;

                    self.pixel_buffer[p_idx] = (out_r.clamp(0.0, 1.0) * 255.0) as u8;
                    self.pixel_buffer[p_idx + 1] = (out_g.clamp(0.0, 1.0) * 255.0) as u8;
                    self.pixel_buffer[p_idx + 2] = (out_b.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }

        // 3. Render Ship-Like Alien Enemy Craft (Bohmian Saucer Corvette & Harmonic Weaver Cruiser)
        for alien in &self.aliens {
            let ax = alien.hull.x;
            let ay = alien.hull.y;
            let r_bound = (alien.hull.radius.ceil() as isize) + 2;

            match alien.kind {
                AlienKind::BohmianSaucer => {
                    // Draw telegraphed lock-on laser sightline before firing
                    if alien.telegraph_timer > 0 {
                        let aim_cos = alien.aim_angle.cos();
                        let aim_sin = alien.aim_angle.sin();
                        for step in 4..28 {
                            let d = step as f32 * 1.1;
                            let gx = ((ax + aim_cos * d).round() as isize).rem_euclid(w as isize) as usize;
                            let gy = ((ay + aim_sin * d).round() as isize).rem_euclid(h as isize) as usize;
                            let p_idx = (gy * w + gx) * 4;
                            self.pixel_buffer[p_idx] = 255;
                            self.pixel_buffer[p_idx + 1] = 85;
                            self.pixel_buffer[p_idx + 2] = 120;
                        }
                    }

                    // Articulated Saucer Corvette: swept wing sponsons, rotating phase emitter nodes, glowing bridge dome
                    let cos_t = alien.hull.theta.cos();
                    let sin_t = alien.hull.theta.sin();
                    let cx_i = ax.round() as isize;
                    let cy_i = ay.round() as isize;

                    for oy in -r_bound..=r_bound {
                        for ox in -r_bound..=r_bound {
                            let wx = cx_i + ox;
                            let wy = cy_i + oy;
                            let dx = wx as f32 - ax;
                            let dy = wy as f32 - ay;
                            // Saucer-local coordinates
                            let lx = dx * cos_t + dy * sin_t;
                            let ly = -dx * sin_t + dy * cos_t;

                            let disc_r = (lx * lx + (ly * 1.32).powi(2)).sqrt();
                            let wing_sponson = lx.abs() <= alien.hull.radius * 1.12
                                && ly.abs() <= 1.45
                                && lx.abs() >= alien.hull.radius * 0.55;
                            if disc_r > alien.hull.radius && !wing_sponson {
                                continue;
                            }

                            let gx = wx.rem_euclid(w as isize) as usize;
                            let gy = wy.rem_euclid(h as isize) as usize;
                            let p_idx = (gy * w + gx) * 4;

                            let col: [u8; 4] = if disc_r < 1.85 {
                                // Bioluminescent cyan-white command bridge canopy
                                [165, 255, 245, 255]
                            } else if (disc_r - 3.3).abs() < 0.65 {
                                // Rotating golden-crimson phase emitter ring
                                let ang = ly.atan2(lx);
                                let node = ((ang * 3.0).cos() > 0.35) as u8;
                                if node == 1 {
                                    [255, 215, 95, 255]
                                } else {
                                    [35, 135, 105, 255]
                                }
                            } else if wing_sponson {
                                // Swept wingtip weapon pods
                                [255, 95, 125, 255]
                            } else {
                                // Armored emerald-slate corvette hull plating
                                [48, 185, 130, 255]
                            };
                            self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&col);
                        }
                    }
                }
                AlienKind::HarmonicWeaver => {
                    // Twin-Prong Acoustic Cruiser oriented along `alien.aim_angle`
                    let cos_a = alien.aim_angle.cos();
                    let sin_a = alien.aim_angle.sin();
                    let cx_i = ax.round() as isize;
                    let cy_i = ay.round() as isize;

                    // Active standing-wave resonance beam between prongs and target rock
                    if alien.telegraph_timer > 0 || alien.fire_cooldown > 34 {
                        for step in 3..16 {
                            let d = step as f32 * 0.9;
                            let wave_w = (step as f32 * 1.1).sin() * 1.2;
                            let wx = ax + cos_a * d - sin_a * wave_w;
                            let wy = ay + sin_a * d + cos_a * wave_w;
                            let gx = (wx.round() as isize).rem_euclid(w as isize) as usize;
                            let gy = (wy.round() as isize).rem_euclid(h as isize) as usize;
                            let p_idx = (gy * w + gx) * 4;
                            self.pixel_buffer[p_idx..p_idx + 4]
                                .copy_from_slice(&[255, 195, 90, 255]);
                        }
                    }

                    for oy in -r_bound..=r_bound {
                        for ox in -r_bound..=r_bound {
                            let wx = cx_i + ox;
                            let wy = cy_i + oy;
                            let dx = wx as f32 - ax;
                            let dy = wy as f32 - ay;
                            // Ship-local frame (+lx = forward along aim_angle)
                            let lx = dx * cos_a + dy * sin_a;
                            let ly = -dx * sin_a + dy * cos_a;

                            let aft_fuselage = lx >= -4.2 && lx <= 1.2 && ly.abs() <= (3.4 - (lx + 1.5).abs() * 0.45);
                            let forward_prongs = lx > 0.5 && lx <= 5.2 && (ly.abs() >= 1.25 && ly.abs() <= 2.65);
                            let engine_pod = lx < -3.2 && lx >= -5.0 && ly.abs() <= 1.6;

                            if !aft_fuselage && !forward_prongs && !engine_pod {
                                continue;
                            }

                            let gx = wx.rem_euclid(w as isize) as usize;
                            let gy = wy.rem_euclid(h as isize) as usize;
                            let p_idx = (gy * w + gx) * 4;

                            let col: [u8; 4] = if engine_pod {
                                // Glowing aft plasma thruster
                                [255, 120, 60, 255]
                            } else if lx.hypot(ly) <= 1.65 {
                                // Harmonic reactor core
                                [255, 235, 130, 255]
                            } else if forward_prongs && lx > 3.4 {
                                // Tuning-fork resonance emitter tips
                                [140, 255, 235, 255]
                            } else if forward_prongs {
                                // Twin forward resonance mandibles
                                [225, 155, 70, 255]
                            } else {
                                // Cruiser hull armor
                                [75, 175, 165, 255]
                            };
                            self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&col);
                        }
                    }
                }
            }
        }

        // 4. Render Oriented Soliton Laser Bolts with Field-Coupled Glow Halos & Mach Shock Wings
        for bolt in &self.bolts {
            let speed = (bolt.vx * bolt.vx + bolt.vy * bolt.vy).sqrt().max(0.01);
            let dir_x = bolt.vx / speed;
            let dir_y = bolt.vy / speed;

            let (core_col, halo_col, tail_len, halo_r): ([f32; 3], [f32; 3], f32, isize) =
                if !bolt.from_player {
                    ([1.0, 0.88, 0.92], [1.0, 0.18, 0.38], 4.5, 4)
                } else if bolt.overcharged {
                    ([1.0, 1.0, 0.90], [1.0, 0.82, 0.25], 7.0, 5)
                } else {
                    ([0.92, 1.0, 1.0], [0.20, 0.88, 1.0], 5.2, 4)
                };

            let cx_i = bolt.x.round() as isize;
            let cy_i = bolt.y.round() as isize;

            for oy in -halo_r..=halo_r {
                for ox in -halo_r..=halo_r {
                    let wx = cx_i + ox;
                    let wy = cy_i + oy;
                    if !periodic && (wx < 0 || wx >= w as isize || wy < 0 || wy >= h as isize) {
                        continue;
                    }
                    let dx = wx as f32 - bolt.x;
                    let dy = wy as f32 - bolt.y;

                    // Project into bolt-aligned coordinates: `along` (+ = nose, - = wake tail), `perp`
                    let along = dx * dir_x + dy * dir_y;
                    let perp = -dx * dir_y + dy * dir_x;

                    let eff_along = if along < 0.0 {
                        along / (tail_len * 0.45)
                    } else {
                        along / 1.45
                    };
                    let d_ellip = (eff_along * eff_along + (perp / 1.35).powi(2)).sqrt();
                    if d_ellip > 2.6 {
                        continue;
                    }

                    let gx = wx.rem_euclid(w as isize) as usize;
                    let gy = wy.rem_euclid(h as isize) as usize;
                    let idx = self.solver.state.config.index(gx, gy);
                    let field_lum = if has_lum {
                        self.solver.state.luminescence[idx].min(2.5)
                    } else {
                        0.5
                    };

                    let core_w = (-d_ellip * d_ellip * 1.65).exp();
                    let halo_w = ((-d_ellip * d_ellip * 0.48).exp()) * (0.65 + 0.35 * field_lum);

                    let p_idx = (gy * w + gx) * 4;
                    let bg_r = self.pixel_buffer[p_idx] as f32 / 255.0;
                    let bg_g = self.pixel_buffer[p_idx + 1] as f32 / 255.0;
                    let bg_b = self.pixel_buffer[p_idx + 2] as f32 / 255.0;

                    let r = bg_r + halo_col[0] * halo_w * 0.78 + core_col[0] * core_w * 0.95;
                    let g = bg_g + halo_col[1] * halo_w * 0.78 + core_col[1] * core_w * 0.95;
                    let b = bg_b + halo_col[2] * halo_w * 0.78 + core_col[2] * core_w * 0.95;

                    self.pixel_buffer[p_idx] = (r.clamp(0.0, 1.0) * 255.0) as u8;
                    self.pixel_buffer[p_idx + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
                    self.pixel_buffer[p_idx + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
                }
            }
        }

        // 5. Overlay Player Wave-Interceptor, Thruster Plume & Antiphase Shield Ring
        let sx = self.ship_x;
        let sy = self.ship_y;
        let cos_t = self.ship_theta.cos();
        let sin_t = self.ship_theta.sin();

        // Render thruster plasma plume when thrusting
        if self.ship_thrusting_timer > 0 {
            for step in 3..=7 {
                let lx = -(step as f32);
                for ly_i in -1isize..=1 {
                    let ly = ly_i as f32 * (1.0 - (step - 3) as f32 * 0.18);
                    let wx = (sx + lx * cos_t - ly * sin_t).round() as isize;
                    let wy = (sy + lx * sin_t + ly * cos_t).round() as isize;
                    let gx = wx.rem_euclid(w as isize) as usize;
                    let gy = wy.rem_euclid(h as isize) as usize;
                    let p_idx = (gy * w + gx) * 4;
                    let plume_col = if step <= 4 {
                        [255, 240, 150, 255]
                    } else {
                        [90, 210, 255, 255]
                    };
                    self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&plume_col);
                }
            }
        }

        for ly in -4isize..=4 {
            for lx in -3isize..=5 {
                let ly_abs = ly.abs() as f32;
                let in_hull = lx >= -2 && lx <= 4 && ly_abs <= (3.6 - lx as f32 * 0.62);
                if in_hull {
                    let wx = (sx + lx as f32 * cos_t - ly as f32 * sin_t).round() as isize;
                    let wy = (sy + lx as f32 * sin_t + ly as f32 * cos_t).round() as isize;
                    let gx = wx.rem_euclid(w as isize) as usize;
                    let gy = wy.rem_euclid(h as isize) as usize;
                    let p_idx = (gy * w + gx) * 4;
                    let ship_col = if lx >= 3 {
                        // Nose lance emitter
                        [255, 255, 255, 255]
                    } else if lx >= 0 && lx <= 1 && ly == 0 {
                        // Cockpit canopy
                        [80, 245, 255, 255]
                    } else if lx == -2 && ly.abs() >= 2 {
                        // Wingtip binaural acoustic probes
                        [255, 200, 90, 255]
                    } else if self.capacitor >= 35.0 {
                        [115, 230, 255, 255]
                    } else {
                        [175, 205, 235, 255]
                    };
                    self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&ship_col);
                }
            }
        }

        if self.shield_visual_timer > 0 {
            let sr = self.ship_radius + 5.0;
            for step in 0..48 {
                let ang = (step as f32 / 48.0) * TAU;
                let wx = (sx + ang.cos() * sr).round() as isize;
                let wy = (sy + ang.sin() * sr).round() as isize;
                let gx = wx.rem_euclid(w as isize) as usize;
                let gy = wy.rem_euclid(h as isize) as usize;
                let p_idx = (gy * w + gx) * 4;
                self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&[130, 255, 220, 255]);
            }
        }
    }
}
