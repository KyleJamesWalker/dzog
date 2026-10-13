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
    /// Hamiltonian Field Capacitor in `[0.0, 100.0]` charged by Antiphase Shield parries & wave harvesting.
    pub capacitor: f32,

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
    pub zero_readback_gpu: bool,
    pub view_mode: FieldViewMode,
    pub topology: WorldTopologyMode,
    pub field_energy: FieldEnergy,
    pub saved_branch: Option<AsteroidsBranchSnapshot>,
    pub gpu_accel: Option<GpuFieldAccelerator>,
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
            capacitor: 50.0,
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
            zero_readback_gpu: false,
            view_mode: FieldViewMode::CompositeArcade,
            topology: WorldTopologyMode::ToroidalPeriodic,
            field_energy: FieldEnergy {
                potential: 0.0,
                kinetic: 0.0,
                total: 0.0,
            },
            saved_branch: None,
            gpu_accel: None,
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

    /// Initialize hardware WGPU compute acceleration.
    pub fn enable_gpu_acceleration(&mut self) -> bool {
        if self.gpu_accel.is_some() {
            return true;
        }
        self.gpu_accel = GpuFieldAccelerator::try_new(
            &self.solver.state,
            &self.solver.material,
            &self.solver.boundary,
            &self.solver.config,
        );
        self.gpu_accel.is_some()
    }

    pub fn toggle_gpu_acceleration(&mut self) -> bool {
        if self.gpu_accel.is_some() {
            self.gpu_accel = None;
            false
        } else {
            self.enable_gpu_acceleration()
        }
    }

    #[inline]
    pub fn is_gpu_accelerated(&self) -> bool {
        self.gpu_accel.is_some()
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

        if let Some(gpu) = &mut self.gpu_accel {
            gpu.sim.set_outer_rule(&gpu.ctx, self.solver.boundary.outer_rule);
        }
    }

    /// Spawn a new wave of mineralogical asteroids and an alien antagonist.
    pub fn spawn_wave(&mut self, wave: u32) {
        self.asteroids.clear();
        self.aliens.clear();
        self.bolts.clear();

        let w = self.width as f32;
        let h = self.height as f32;
        let count = (4 + wave as usize).min(9);

        let classes = [
            MineralClass::PiezoQuartz,
            MineralClass::ObsidianAnvil,
            MineralClass::BismuthCrystal,
        ];

        for i in 0..count {
            let angle = (i as f32 / count as f32) * TAU + 0.35;
            let dist = w.min(h) * 0.34;
            let ax = (w * 0.5 + angle.cos() * dist).rem_euclid(w);
            let ay = (h * 0.5 + angle.sin() * dist).rem_euclid(h);

            let class = classes[i % classes.len()];
            let id = self.next_body_id;
            self.next_body_id += 1;

            let mut rock = CohesiveBody::new_asteroid(
                id,
                ax,
                ay,
                6.8,
                6 + (i % 3),
                (i as u32 + 1) * 7919 + wave * 131,
                class,
            );
            let drift_ang = angle + 1.35;
            let speed = 0.14 + (i % 3) as f32 * 0.04;
            rock.vx = drift_ang.cos() * speed;
            rock.vy = drift_ang.sin() * speed;
            rock.omega = if i % 2 == 0 { 0.018 } else { -0.022 };
            self.asteroids.push(rock);
        }

        // Spawn a Bohmian Saucer antagonist on wave 1+, plus a Harmonic Weaver on wave 2+
        self.spawn_alien(AlienKind::BohmianSaucer);
        if wave >= 2 {
            self.spawn_alien(AlienKind::HarmonicWeaver);
        }
    }

    /// Spawn a field-coupled alien antagonist at the arena perimeter.
    pub fn spawn_alien(&mut self, kind: AlienKind) {
        let id = self.next_body_id;
        self.next_body_id += 1;
        let (x, y) = match kind {
            AlienKind::BohmianSaucer => (14.0, 14.0),
            AlienKind::HarmonicWeaver => (self.width as f32 - 14.0, self.height as f32 - 14.0),
        };
        let mut hull = CohesiveBody::new_saucer(id, x, y, 4.8);
        hull.vx = 0.22;
        hull.vy = 0.12;
        hull.omega = 0.03;

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
        let exhaust_angle = self.ship_theta + PI;
        let nozzle_x = self.ship_x + exhaust_angle.cos() * (self.ship_radius + 1.2);
        let nozzle_y = self.ship_y + exhaust_angle.sin() * (self.ship_radius + 1.2);

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

            // Use Zero-Readback GPU Force Reduction when enabled, or CPU contour integrator
            if self.zero_readback_gpu && self.gpu_accel.is_some() {
                let mut queries = Vec::with_capacity(self.asteroids.len());
                for rock in &self.asteroids {
                    queries.push(GpuBodyQuery {
                        x: rock.x,
                        y: rock.y,
                        radius: rock.radius,
                        yield_pressure: rock.material.yield_pressure,
                    });
                }
                let reductions = {
                    let gpu = self.gpu_accel.as_ref().unwrap();
                    gpu.sim.reduce_bodies_gpu(&gpu.ctx, &queries)
                };
                for (rock, red) in self.asteroids.iter_mut().zip(reductions.iter()) {
                    rock.vx = (rock.vx + red.fx / rock.mass).clamp(-0.65, 0.65);
                    rock.vy = (rock.vy + red.fy / rock.mass).clamp(-0.65, 0.65);
                    rock.omega = (rock.omega + red.tau_z / rock.inertia).clamp(-0.14, 0.14);
                }
            } else {
                for rock in &mut self.asteroids {
                    rock.integrate_field_forces(&self.solver.state, 1.0, periodic);
                    if self.topology == WorldTopologyMode::GravityWellArena {
                        let (agx, agy) = self.gravity.acceleration_at(rock.x, rock.y, periodic);
                        rock.vx = (rock.vx + agx * 0.20).clamp(-0.65, 0.65);
                        rock.vy = (rock.vy + agy * 0.20).clamp(-0.65, 0.65);
                    }
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
                            let aim = dy.atan2(dx);
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
                                // PiezoQuartz emits an extra-strong piezo shock ring on fracture!
                                let amp = if class == MineralClass::PiezoQuartz { 4.2 } else { 3.0 };
                                let blast = GaussianImpulse::new(removed.x, removed.y, 2.6, amp);
                                blast.inject_wrapped(&mut self.solver.state, periodic);
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
                if self.lives == 0 {
                    self.in_play = false;
                    event = AsteroidEvent::GameOver;
                    break;
                } else {
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

        // 4. Synchronize all rotating bodies into the lattice and advance FDTD substeps
        self.sync_all_bodies_to_lattice();

        if let Some(gpu) = &self.gpu_accel {
            let f_substeps = self.solver.config.substeps_per_macro;
            gpu.sim.upload_state(&gpu.ctx, &self.solver.state);
            gpu.sim.upload_material(&gpu.ctx, &self.solver.material);
            gpu.sim.upload_boundary(&gpu.ctx, &self.solver.boundary);
            gpu.sim.step(&gpu.ctx, f_substeps);
            self.solver.state = gpu.sim.readback_state(&gpu.ctx);
            self.solver.current_step += f_substeps;
            self.solver.current_time += self.solver.config.dt * f_substeps as f32;
        } else {
            self.solver.advance_macro();
        }

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

        if self.pixel_buffer.len() != w * h * 4 {
            self.pixel_buffer.resize(w * h * 4, 0);
        }

        // 1. Background field projection according to active `FieldViewMode`
        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let p = self.solver.state.pressure[idx];
                let vx = self.solver.state.vx[idx];
                let vy = self.solver.state.vy[idx];
                let rho = self.solver.material.density[idx];
                let k = self.solver.material.bulk_modulus[idx];
                let coh = self.solver.material.cohesion[idx];

                let (mut r, mut g, mut b): (f32, f32, f32) = match self.view_mode {
                    FieldViewMode::CompositeArcade => {
                        let p_pos = p.max(0.0);
                        let p_neg = (-p).max(0.0);
                        let v_mag = (vx * vx + vy * vy).sqrt();
                        (
                            0.02 + p_pos * 0.45 + v_mag * 0.15,
                            0.03 + p_pos * 0.25 + p_neg * 0.35 + v_mag * 0.25,
                            0.06 + p_neg * 0.65 + p_pos * 0.20,
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
                            0.03 + p_pos * 0.65 + q_well * 0.55,
                            0.03 + q_well * 0.85,
                            0.06 + p_neg * 0.70 + q_well * 0.40,
                        )
                    }
                    FieldViewMode::VelocityAndFlux => {
                        let v_mag = (vx * vx + vy * vy).sqrt();
                        let flux = (p * vx).hypot(p * vy);
                        let phi_g = (-self.gravity.potential[idx]).max(0.0);
                        (
                            0.03 + flux * 0.65 + phi_g * 0.35,
                            0.04 + v_mag * 0.75,
                            0.07 + v_mag * 0.45 + phi_g * 0.75,
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

                // Render crystal material & glowing internal cleavage veins
                if coh >= 0.15 {
                    let p_glow = p.abs().min(2.0) * 0.25;
                    if coh < 0.55 {
                        // Glowing semi-permeable fracture waveguide vein!
                        r = 0.95;
                        g = 0.88;
                        b = 0.35 + p_glow;
                    } else if rho < 0.90 {
                        // PiezoQuartz Geode (Cyan-Violet crystal)
                        r = 0.25 + p_glow;
                        g = 0.78 + p_glow * 0.5;
                        b = 0.95;
                    } else if rho > 1.60 {
                        // Obsidian Anvil (Dark metallic crimson-slate)
                        r = 0.48 + p_glow;
                        g = 0.26;
                        b = 0.34 + p_glow * 0.5;
                    } else if k > 1.55 {
                        // Alien Resonator Hull (Emerald-Gold)
                        r = 0.20 + p_glow;
                        g = 0.92;
                        b = 0.55;
                    } else {
                        // Bismuth Metamaterial (Iridescent Amber-Teal)
                        r = 0.85;
                        g = 0.58 + p_glow * 0.4;
                        b = 0.25 + p_glow;
                    }
                }

                let p_idx = (y * w + x) * 4;
                self.pixel_buffer[p_idx] = (r.clamp(0.0, 1.0) * 255.0) as u8;
                self.pixel_buffer[p_idx + 1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
                self.pixel_buffer[p_idx + 2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
                self.pixel_buffer[p_idx + 3] = 255;
            }
        }

        // 2. Overlay Soliton Bolts
        for bolt in &self.bolts {
            let bx = (bolt.x.round() as isize).rem_euclid(w as isize) as usize;
            let by = (bolt.y.round() as isize).rem_euclid(h as isize) as usize;
            let color: [u8; 4] = if !bolt.from_player {
                [255, 70, 90, 255]
            } else if bolt.overcharged {
                [255, 250, 140, 255]
            } else {
                [120, 245, 255, 255]
            };
            for oy in -1isize..=1 {
                for ox in -1isize..=1 {
                    let gx = (bx as isize + ox).rem_euclid(w as isize) as usize;
                    let gy = (by as isize + oy).rem_euclid(h as isize) as usize;
                    let p_idx = (gy * w + gx) * 4;
                    self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&color);
                }
            }
        }

        // 3. Overlay Player Wave-Interceptor & Antiphase Shield Ring
        let sx = self.ship_x;
        let sy = self.ship_y;
        let cos_t = self.ship_theta.cos();
        let sin_t = self.ship_theta.sin();

        for ly in -3isize..=3 {
            for lx in -3isize..=4 {
                // Sleek delta-wing ship silhouette in ship-local coordinates
                let in_hull = lx >= -2 && lx <= 3 && (ly.abs() as f32) <= (3.2 - lx as f32 * 0.65);
                if in_hull {
                    let wx = (sx + lx as f32 * cos_t - ly as f32 * sin_t).round() as isize;
                    let wy = (sy + lx as f32 * sin_t + ly as f32 * cos_t).round() as isize;
                    let gx = wx.rem_euclid(w as isize) as usize;
                    let gy = wy.rem_euclid(h as isize) as usize;
                    let p_idx = (gy * w + gx) * 4;
                    let ship_col = if lx >= 2 {
                        [255, 255, 255, 255]
                    } else if self.capacitor >= 35.0 {
                        [110, 235, 255, 255]
                    } else {
                        [180, 210, 240, 255]
                    };
                    self.pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&ship_col);
                }
            }
        }

        if self.shield_visual_timer > 0 {
            let sr = self.ship_radius + 5.0;
            for step in 0..36 {
                let ang = (step as f32 / 36.0) * TAU;
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
