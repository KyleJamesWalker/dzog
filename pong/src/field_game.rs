//! Field-First Breakout Pong: Single-player Breakout on the Dzog acoustic lattice.
//!
//! Demonstrates Dzoglang's core philosophy (`philosophy.md` & `readme.md`):
//! - Paddles and Bricks are physical acoustic boundaries and heterogeneous impedance media on the Yee lattice.
//! - Point-of-Impact Dynamic Structural Fracturing:
//!   - Bricks track a local 2D cell damage field `damage[4][12]` and contact impact vectors.
//!   - 1st hit: Carves directional fracture fissures from the exact contact point $(cx, cy)$ along the stress tensor,
//!     opening semi-permeable wave channels through the brick's heterogeneous medium ($\rho, K, \gamma$).
//!   - 2nd hit (or Acoustic Wave Cleave): Cavitation rupture! Clears boundary, relaxes material to ambient fluid,
//!     and (when field coupling is enabled) injects an explosive sonic burst into the lattice.
//! - Acoustic Wave-Induced Cleaving: Constructive interference ($|p| > p_{\text{cleave}}$) shatters weakened bricks
//!   without the ball touching them, triggering domino chain reactions.
//! - Bohm-Hiley Quantum Potential ($Q = -\nabla^2 R / R$) & Radiation Pressure ($-\nabla p$) wave-guided ball dynamics.
//! - Counterfactual Branching (`[B]`/`[U]`), Multi-View Inspection (`[V]`), and Mouse Perturbation.

use dzog_core::*;

/// Perceptual inspection view mode (`philosophy.md` §Multiple Views).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldViewMode {
    /// Default arcade view: pressure waves, kinetic shimmer, and procedural fracture veins.
    CompositeArcade,
    /// Scalar pressure p(x,y) and Bohmian Quantum Potential Q(x,y) guidance ridges.
    PressureAndBohmian,
    /// Particle velocity |v| and instantaneous acoustic energy flux (Poynting vector p*v).
    VelocityAndFlux,
    /// Material acoustic impedance Z(x,y) = sqrt(rho * K) and structural cohesion C(x,y).
    ImpedanceAndCohesion,
}

#[allow(dead_code)]
impl FieldViewMode {
    pub fn next(self) -> Self {
        match self {
            Self::CompositeArcade => Self::PressureAndBohmian,
            Self::PressureAndBohmian => Self::VelocityAndFlux,
            Self::VelocityAndFlux => Self::ImpedanceAndCohesion,
            Self::ImpedanceAndCohesion => Self::CompositeArcade,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::CompositeArcade => "Arcade + Wavefield",
            Self::PressureAndBohmian => "Pressure p & Bohmian Q",
            Self::VelocityAndFlux => "Velocity v & Energy Flux",
            Self::ImpedanceAndCohesion => "Impedance Z & Cohesion C",
        }
    }
}

/// Physical acoustic material properties per brick row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrickMaterialSpec {
    pub name: &'static str,
    pub rho: f32,
    pub bulk_modulus: f32,
    pub damping: f32,
    pub cleave_pressure: f32,
}

impl BrickMaterialSpec {
    pub const fn for_row(row: u8) -> Self {
        match row {
            // Row 0: Crystal Quartz — Fast wave speed (c ~ 1.39), low loss, rings with high-pitch modes
            0 => Self {
                name: "Crystal Quartz",
                rho: 0.75,
                bulk_modulus: 1.45,
                damping: 0.001,
                cleave_pressure: 2.05,
            },
            // Row 1: Basalt Anvil — High impedance (Z ~ 1.96), reflects & channels focused shockwaves
            1 => Self {
                name: "Basalt Anvil",
                rho: 2.20,
                bulk_modulus: 1.75,
                damping: 0.004,
                cleave_pressure: 2.25,
            },
            // Row 2: Viscous Metamaterial — Slow wave speed (c ~ 0.68), refractive acoustic lens & damper
            2 => Self {
                name: "Viscous Lens",
                rho: 1.30,
                bulk_modulus: 0.60,
                damping: 0.055,
                cleave_pressure: 1.95,
            },
            // Row 3+: Sonic Resonator — Tuned cavity impedance, brittle under constructive wave interference
            _ => Self {
                name: "Sonic Resonator",
                rho: 0.90,
                bulk_modulus: 1.15,
                damping: 0.002,
                cleave_pressure: 1.30,
            },
        }
    }
}

/// A breakable brick on the acoustic lattice with local cell damage and contact fracture state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brick {
    /// Left column on the grid.
    pub x: usize,
    /// Top row on the grid.
    pub y: usize,
    /// Width in cells (standard: 12).
    pub w: usize,
    /// Height in cells (standard: 4).
    pub h: usize,
    /// Structural state before full cavitation rupture (2 = intact, 1 = fractured/semi-permeable, 0 = shattered).
    pub hits_left: u8,
    /// Row index (0..3) determining physical material $(\rho, K, \gamma)$ and palette.
    pub row: u8,
    /// Local 2D cell damage field in [0.0, 1.0] (0.0 = pristine solid, 1.0 = fractured fissure).
    pub damage: [[f32; 12]; 4],
    /// Last point-of-contact in local brick coordinates (lx, ly) and inward impact normal (nx, ny).
    pub last_contact: Option<(f32, f32, f32, f32)>,
}

#[allow(dead_code)]
impl Brick {
    pub fn new(x: usize, y: usize, w: usize, h: usize, row: u8) -> Self {
        Self {
            x,
            y,
            w,
            h,
            hits_left: 2,
            row,
            damage: [[0.0; 12]; 4],
            last_contact: None,
        }
    }

    /// Whether this brick is still present on the lattice.
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.hits_left > 0
    }

    /// Material specification for this brick's row.
    #[inline]
    pub fn material_spec(&self) -> BrickMaterialSpec {
        BrickMaterialSpec::for_row(self.row)
    }

    /// Mean structural cohesion across all cells of this brick in [0.0, 1.0].
    pub fn mean_cohesion(&self) -> f32 {
        let mut sum = 0.0f32;
        for ry in 0..4 {
            for rx in 0..12 {
                sum += 1.0 - self.damage[ry][rx].clamp(0.0, 1.0);
            }
        }
        sum / 48.0
    }

    /// Carve procedural fracture fissures radiating from the exact world contact point `(cx, cy)`
    /// along the impact stress direction `(-normal_x, -normal_y)`.
    pub fn apply_contact_fracture(&mut self, cx: f32, cy: f32, normal_x: f32, normal_y: f32) {
        let lx = (cx - self.x as f32).clamp(0.0, (self.w.saturating_sub(1)) as f32);
        let ly = (cy - self.y as f32).clamp(0.0, (self.h.saturating_sub(1)) as f32);

        // Inward stress propagation vector (opposite to outward rebound normal)
        let dir_x = -normal_x;
        let mut dir_y = -normal_y;
        if dir_x.abs() < 0.01 && dir_y.abs() < 0.01 {
            dir_y = -1.0;
        }
        self.last_contact = Some((lx, ly, dir_x, dir_y));

        // 1. Impact crater around contact point (cx, cy)
        for ry in 0..4 {
            for rx in 0..12 {
                let dx = rx as f32 - lx;
                let dy = ry as f32 - ly;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist <= 1.6 {
                    let crater_dmg = (1.0 - dist / 2.0).clamp(0.55, 1.0);
                    self.damage[ry][rx] = self.damage[ry][rx].max(crater_dmg);
                }
            }
        }

        // 2. Trace 3 fracture rays from (lx, ly): primary stress axis + two shear branches
        let base_angle = dir_y.atan2(dir_x);
        let branch_offsets = [0.0f32, 0.52f32, -0.52f32];

        for (b_idx, &ang_offset) in branch_offsets.iter().enumerate() {
            let angle = base_angle + ang_offset;
            let vx = angle.cos();
            let vy = angle.sin();
            let max_steps = if b_idx == 0 { 24 } else { 16 };

            for step in 0..max_steps {
                let t = step as f32 * 0.45;
                // Deterministic jagged shear perturbation
                let jitter = ((step as f32 * 1.7 + b_idx as f32 * 2.3).sin()) * 0.35;
                let px = (lx + vx * t - vy * jitter).round() as isize;
                let py = (ly + vy * t + vx * jitter).round() as isize;

                if px >= 0 && px < 12 && py >= 0 && py < 4 {
                    let intensity = if b_idx == 0 { 0.95 } else { 0.80 };
                    self.damage[py as usize][px as usize] =
                        self.damage[py as usize][px as usize].max(intensity);
                }
            }
        }
    }
}

/// Complete snapshot of a Breakout game & field situation for counterfactual branching (`[B]`/`[U]`).
#[derive(Debug, Clone)]
pub struct GameBranchSnapshot {
    pub checkpoint: Checkpoint,
    pub bricks: Vec<Brick>,
    pub bricks_remaining: usize,
    pub paddle_x: f32,
    pub paddle_vx: f32,
    pub ball_x: f32,
    pub ball_y: f32,
    pub ball_vx: f32,
    pub ball_vy: f32,
    pub ball_attached: bool,
    pub lives: u32,
    pub score: u32,
    pub combo: u32,
}

/// Simulation event emitted during a physics step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepEvent {
    None,
    /// Brick was struck for the 1st time, carving directional contact stress fissures.
    BrickCracked { brick_idx: usize, row: u8 },
    /// Brick was shattered (by ball impact or acoustic wave resonance).
    BrickBroken { brick_idx: usize, row: u8, remaining: usize },
    /// Ball rebounded off the player's paddle.
    PaddleHit,
    /// Ball bounced off the court boundary walls.
    WallHit,
    /// Ball fell past the paddle into the drain.
    LifeLost { remaining_lives: u32 },
    /// Player lost all lives.
    GameOver,
    /// All bricks cleared!
    Victory,
}

pub struct BreakoutFieldGame {
    pub solver: AcousticSolver,
    pub width: usize,
    pub height: usize,

    // Bricks
    pub bricks: Vec<Brick>,
    pub bricks_remaining: usize,
    pub acoustic_cleaves: u32,

    // Paddle kinematics & lattice boundary
    pub paddle_x: f32,
    pub paddle_y: usize,
    pub paddle_w: usize,
    pub paddle_h: usize,
    pub paddle_vx: f32,

    // Ball kinematics
    pub ball_x: f32,
    pub ball_y: f32,
    pub ball_vx: f32,
    pub ball_vy: f32,
    pub ball_radius: f32,
    pub ball_attached: bool,

    // Game state
    pub in_play: bool,
    pub lives: u32,
    pub score: u32,
    pub high_score: u32,
    pub combo: u32,

    // Engine mechanics & inspection
    pub wave_force_enabled: bool,
    pub view_mode: FieldViewMode,
    pub field_energy: FieldEnergy,
    pub saved_branch: Option<GameBranchSnapshot>,

    // Framebuffer
    pub pixel_buffer: Vec<u8>,
}

#[allow(dead_code)]
pub type SolitonFieldGame = BreakoutFieldGame;

#[allow(dead_code)]
impl BreakoutFieldGame {
    pub fn new(width: usize, height: usize) -> Self {
        let grid = GridConfig::new(width, height, 1.0, 1.0);
        let state = AcousticState::new(grid);

        // Ambient fluid medium with slight damping; reserve CFL headroom for high-speed Quartz (c = sqrt(1.75/0.75))
        let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.007);
        // Seed corner cell with max wave-speed bound so compute_cfl_dt accounts for all heterogeneous brick rows
        material.density[0] = 0.70;
        material.bulk_modulus[0] = 1.80;
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        material.density[0] = 1.0;
        material.bulk_modulus[0] = 1.0;

        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);

        let nl = NonlinearLaw {
            beta: 0.55,
            alpha: 0.35,
            threshold_decay: 0.015,
        };
        let config = SolverConfig::new(dt, 0.7, 4).with_nonlinearity(nl);
        let solver = AcousticSolver::new(state, material, boundary, config).unwrap();

        let paddle_w = 20;
        let paddle_h = 3;
        let paddle_y = height.saturating_sub(8);
        let paddle_x = (width as f32 - paddle_w as f32) / 2.0;

        let mut game = Self {
            solver,
            width,
            height,
            bricks: Vec::new(),
            bricks_remaining: 0,
            acoustic_cleaves: 0,
            paddle_x,
            paddle_y,
            paddle_w,
            paddle_h,
            paddle_vx: 0.0,
            ball_x: paddle_x + paddle_w as f32 / 2.0,
            ball_y: paddle_y as f32 - 2.5,
            ball_vx: 0.0,
            ball_vy: 0.0,
            ball_radius: 1.6,
            ball_attached: true,
            in_play: true,
            lives: 3,
            score: 0,
            high_score: 0,
            combo: 0,
            wave_force_enabled: true,
            view_mode: FieldViewMode::CompositeArcade,
            field_energy: FieldEnergy {
                potential: 0.0,
                kinetic: 0.0,
                total: 0.0,
            },
            saved_branch: None,
            pixel_buffer: vec![0; width * height * 4],
        };

        game.init_bricks();
        game.sync_all_brick_boundaries();
        game.sync_paddle_boundaries();
        game
    }

    /// Layout 4 rows of 8 breakable bricks (32 bricks total).
    pub fn init_bricks(&mut self) {
        self.bricks.clear();

        let num_cols = 8;
        let num_rows = 4;
        let brick_w = 12;
        let brick_h = 4;
        let gap_x = 2;
        let gap_y = 3;
        let start_x = 9;
        let start_y = 10;

        for r in 0..num_rows {
            let y = start_y + r * (brick_h + gap_y);
            for c in 0..num_cols {
                let x = start_x + c * (brick_w + gap_x);
                self.bricks.push(Brick::new(x, y, brick_w, brick_h, r as u8));
            }
        }
        self.bricks_remaining = self.bricks.len();
    }

    /// Reset court state for a new game or level.
    pub fn reset_game(&mut self) {
        self.solver.state.clear();
        self.solver.boundary.clear_interior();
        let grid = self.solver.state.config;
        self.solver.material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.007);

        self.init_bricks();
        self.sync_all_brick_boundaries();

        self.lives = 3;
        self.score = 0;
        self.combo = 0;
        self.acoustic_cleaves = 0;
        self.in_play = true;

        self.paddle_x = (self.width as f32 - self.paddle_w as f32) / 2.0;
        self.paddle_vx = 0.0;
        self.sync_paddle_boundaries();

        self.ball_attached = true;
        self.ball_x = self.paddle_x + self.paddle_w as f32 / 2.0;
        self.ball_y = self.paddle_y as f32 - self.ball_radius - 0.8;
        self.ball_vx = 0.0;
        self.ball_vy = 0.0;
    }

    /// Synchronize all living bricks' heterogeneous material properties $(\rho, K, \gamma)$,
    /// cohesion field, and boundary walls on the Yee lattice.
    pub fn sync_all_brick_boundaries(&mut self) {
        let bricks_clone = self.bricks.clone();
        for b in &bricks_clone {
            if b.is_alive() {
                self.sync_single_brick_to_lattice(b);
            }
        }
    }

    /// Write a single brick's material and fracture-aware boundary state into the solver lattice.
    pub fn sync_single_brick_to_lattice(&mut self, b: &Brick) {
        let w = self.width;
        let h = self.height;
        let spec = b.material_spec();

        let x_min = b.x.saturating_sub(1);
        let x_max = (b.x + b.w - 1).min(w - 1);
        let y_min = b.y.saturating_sub(1);
        let y_max = (b.y + b.h - 1).min(h - 1);

        // 1. Set heterogeneous material parameters and cell cohesion
        for ry in 0..b.h.min(4) {
            for rx in 0..b.w.min(12) {
                let gx = (b.x + rx).min(w - 1);
                let gy = (b.y + ry).min(h - 1);
                let idx = self.solver.state.config.index(gx, gy);
                let dmg = b.damage[ry][rx].clamp(0.0, 1.0);
                let coh = if b.hits_left == 2 {
                    1.0
                } else {
                    (1.0 - dmg).clamp(0.05, 0.85)
                };

                self.solver.material.density[idx] = spec.rho;
                self.solver.material.bulk_modulus[idx] = spec.bulk_modulus;
                self.solver.material.damping[idx] = spec.damping;
                self.solver.material.cohesion[idx] = coh;
                self.solver.material.yield_pressure[idx] = spec.cleave_pressure;
            }
        }

        // 2. Enforce rigid walls for intact cells; open semi-permeable channels along fracture veins!
        if b.hits_left == 2 {
            self.solver
                .boundary
                .set_moving_rect(x_min, x_max, y_min, y_max, 0.0, 0.0);
        } else if b.hits_left == 1 {
            self.solver
                .boundary
                .set_moving_rect(x_min, x_max, y_min, y_max, 0.0, 0.0);
            // Unblock fractured cells so acoustic waves refract and tunnel through cracks
            for ry in 0..b.h.min(4) {
                for rx in 0..b.w.min(12) {
                    if b.damage[ry][rx] >= 0.55 {
                        let gx = (b.x + rx).min(w - 1);
                        let gy = (b.y + ry).min(h - 1);
                        let idx = self.solver.state.config.index(gx, gy);
                        self.solver.boundary.wall_east[idx] = false;
                        self.solver.boundary.wall_north[idx] = false;
                        if gx > 0 {
                            let idx_w = self.solver.state.config.index(gx - 1, gy);
                            self.solver.boundary.wall_east[idx_w] = false;
                        }
                        if gy > 0 {
                            let idx_s = self.solver.state.config.index(gx, gy - 1);
                            self.solver.boundary.wall_north[idx_s] = false;
                        }
                    }
                }
            }
        }
    }

    /// Clear a specific brick's boundary and relax its material back to ambient fluid when destroyed.
    pub fn clear_brick_boundary(&mut self, b: &Brick) {
        let w = self.width;
        let h = self.height;
        let x_min = b.x.saturating_sub(1);
        let x_max = (b.x + b.w - 1).min(w - 1);
        let y_min = b.y.saturating_sub(1);
        let y_max = (b.y + b.h - 1).min(h - 1);
        self.solver.boundary.clear_region(x_min, x_max, y_min, y_max);
        self.solver.material.set_cohesive_region(
            b.x,
            (b.x + b.w).min(w),
            b.y,
            (b.y + b.h).min(h),
            1.0,
            1.0,
            0.007,
            0.0,
            f32::INFINITY,
        );
    }

    /// Update paddle position as a moving boundary on the Yee lattice.
    pub fn sync_paddle_boundaries(&mut self) {
        let w = self.width;
        let h = self.height;
        let py0 = self.paddle_y.saturating_sub(1);
        let py1 = (self.paddle_y + self.paddle_h - 1).min(h - 1);

        // Clear paddle row
        self.solver.boundary.clear_region(2, w - 3, py0, py1);

        // Re-apply moving paddle rectangle
        let px0 = (self.paddle_x as usize).clamp(2, w.saturating_sub(self.paddle_w + 3));
        let px1 = (px0 + self.paddle_w - 1).min(w - 3);

        self.solver
            .boundary
            .set_moving_rect(px0, px1, py0, py1, self.paddle_vx, 0.0);
    }

    /// Move player paddle horizontally by `dx` cells.
    pub fn move_paddle(&mut self, dx: f32) {
        let max_x = (self.width - self.paddle_w - 3) as f32;
        let min_x = 3.0f32;
        let new_x = (self.paddle_x + dx).clamp(min_x, max_x);

        self.paddle_vx = (new_x - self.paddle_x) * 0.8;
        self.paddle_x = new_x;

        if self.ball_attached {
            self.ball_x = self.paddle_x + self.paddle_w as f32 / 2.0;
        }

        self.sync_paddle_boundaries();
    }

    /// Set paddle position directly (e.g. from mouse).
    pub fn set_paddle_x(&mut self, target_x: f32) {
        let max_x = (self.width - self.paddle_w - 3) as f32;
        let min_x = 3.0f32;
        let new_x = (target_x - self.paddle_w as f32 / 2.0).clamp(min_x, max_x);

        self.paddle_vx = (new_x - self.paddle_x) * 0.5;
        self.paddle_x = new_x;

        if self.ball_attached {
            self.ball_x = self.paddle_x + self.paddle_w as f32 / 2.0;
        }

        self.sync_paddle_boundaries();
    }

    /// Serve / launch the ball upwards from the paddle.
    pub fn serve(&mut self) {
        if !self.ball_attached {
            return;
        }

        self.ball_attached = false;
        let launch_vx = (self.paddle_vx * 0.4 + 0.15).clamp(-0.45, 0.45);
        let launch_vy = -0.72f32;

        self.ball_vx = launch_vx;
        self.ball_vy = launch_vy;

        if self.wave_force_enabled {
            let impulse = GaussianImpulse::new(self.ball_x, self.ball_y, 2.4, 1.4);
            impulse.inject(&mut self.solver.state);
        }
    }

    /// Toggle counterfactual field coupling (sonic blast, wave radiation & Bohmian force, acoustic cleaving).
    pub fn toggle_wave_force(&mut self) {
        self.wave_force_enabled = !self.wave_force_enabled;
    }

    /// Cycle through perceptual field view modes (`[V]`).
    pub fn cycle_view_mode(&mut self) {
        self.view_mode = self.view_mode.next();
    }

    /// Save a complete situation branch checkpoint (`[B]`).
    pub fn save_branch_checkpoint(&mut self) {
        self.saved_branch = Some(GameBranchSnapshot {
            checkpoint: Checkpoint::capture(&self.solver),
            bricks: self.bricks.clone(),
            bricks_remaining: self.bricks_remaining,
            paddle_x: self.paddle_x,
            paddle_vx: self.paddle_vx,
            ball_x: self.ball_x,
            ball_y: self.ball_y,
            ball_vx: self.ball_vx,
            ball_vy: self.ball_vy,
            ball_attached: self.ball_attached,
            lives: self.lives,
            score: self.score,
            combo: self.combo,
        });
    }

    /// Restore a saved situation branch checkpoint (`[U]`) to compare counterfactual outcomes.
    pub fn restore_branch_checkpoint(&mut self) -> bool {
        if let Some(branch) = &self.saved_branch {
            if let Ok(restored_solver) = branch.checkpoint.restore() {
                self.solver = restored_solver;
                self.bricks = branch.bricks.clone();
                self.bricks_remaining = branch.bricks_remaining;
                self.paddle_x = branch.paddle_x;
                self.paddle_vx = branch.paddle_vx;
                self.ball_x = branch.ball_x;
                self.ball_y = branch.ball_y;
                self.ball_vx = branch.ball_vx;
                self.ball_vy = branch.ball_vy;
                self.ball_attached = branch.ball_attached;
                self.lives = branch.lives;
                self.score = branch.score;
                self.combo = branch.combo;
                self.in_play = true;
                self.field_energy =
                    FieldEnergy::compute(&self.solver.state, &self.solver.material);
                return true;
            }
        }
        false
    }

    /// Interactive mouse utterance: inject an acoustic pressure disturbance at `(gx, gy)`.
    pub fn inject_mouse_impulse(&mut self, gx: f32, gy: f32, amplitude: f32) {
        let cx = gx.clamp(3.0, (self.width.saturating_sub(4)) as f32);
        let cy = gy.clamp(3.0, (self.height.saturating_sub(4)) as f32);
        let impulse = GaussianImpulse::new(cx, cy, 2.5, amplitude);
        impulse.inject(&mut self.solver.state);
    }

    /// Interactive mouse utterance: paint a localized high-impedance acoustic reflector lens at `(gx, gy)`.
    pub fn paint_acoustic_reflector(&mut self, gx: usize, gy: usize) {
        let x0 = gx.clamp(3, self.width.saturating_sub(5));
        let y0 = gy.clamp(3, self.paddle_y.saturating_sub(4));
        self.solver
            .material
            .set_cohesive_region(x0, x0 + 2, y0, y0 + 2, 2.2, 1.75, 0.002, 0.9, 3.0);
        self.solver
            .boundary
            .set_moving_rect(x0, x0 + 1, y0, y0 + 1, 0.0, 0.0);
    }

    /// Advance game physics and Dzog field substeps.
    pub fn step(&mut self) -> StepEvent {
        if !self.in_play {
            return StepEvent::None;
        }

        let w = self.width as f32;
        let h = self.height as f32;
        let sub_steps = 4;
        let dt_sub = 1.0 / sub_steps as f32;
        let mut event = StepEvent::None;

        for _ in 0..sub_steps {
            if self.ball_attached {
                self.ball_x = self.paddle_x + self.paddle_w as f32 / 2.0;
                self.ball_y = self.paddle_y as f32 - self.ball_radius - 0.8;
                break;
            }

            // 1. Acoustic radiation pressure (-grad p) + Bohmian Quantum Potential (-grad Q) coupling:
            if self.wave_force_enabled {
                let bx_round = self.ball_x.round() as isize;
                let by_round = self.ball_y.round() as isize;
                if bx_round >= 2
                    && bx_round < (self.width as isize - 2)
                    && by_round >= 2
                    && by_round < (self.height as isize - 2)
                {
                    let x = bx_round as usize;
                    let y = by_round as usize;
                    let (grad_x, grad_y) = self.solver.state.pressure_gradient_at(x, y);
                    let (q_fx, q_fy) = self.solver.state.quantum_force_at(x, y);

                    let k_acoustic = 0.035;
                    let k_bohmian = 0.004;
                    self.ball_vx += (-grad_x * k_acoustic + q_fx * k_bohmian) * dt_sub;
                    self.ball_vy += (-grad_y * k_acoustic + q_fy * k_bohmian) * dt_sub;
                }
            }

            // 2. Continuous kinematic step
            self.ball_x += self.ball_vx * dt_sub;
            self.ball_y += self.ball_vy * dt_sub;

            // 3. Wall collisions (Left, Right, Top)
            if self.ball_x - self.ball_radius <= 2.0 {
                self.ball_x = 2.0 + self.ball_radius;
                self.ball_vx = self.ball_vx.abs();
                if self.wave_force_enabled {
                    let impact = GaussianImpulse::new(2.0, self.ball_y, 2.0, 1.2);
                    impact.inject(&mut self.solver.state);
                }
                if event == StepEvent::None {
                    event = StepEvent::WallHit;
                }
            } else if self.ball_x + self.ball_radius >= w - 3.0 {
                self.ball_x = w - 3.0 - self.ball_radius;
                self.ball_vx = -self.ball_vx.abs();
                if self.wave_force_enabled {
                    let impact = GaussianImpulse::new(w - 3.0, self.ball_y, 2.0, 1.2);
                    impact.inject(&mut self.solver.state);
                }
                if event == StepEvent::None {
                    event = StepEvent::WallHit;
                }
            }

            if self.ball_y - self.ball_radius <= 2.0 {
                self.ball_y = 2.0 + self.ball_radius;
                self.ball_vy = self.ball_vy.abs();
                if self.wave_force_enabled {
                    let impact = GaussianImpulse::new(self.ball_x, 2.0, 2.2, 1.4);
                    impact.inject(&mut self.solver.state);
                }
                if event == StepEvent::None {
                    event = StepEvent::WallHit;
                }
            }

            // 4. Paddle collision
            let px = self.paddle_x;
            let pw = self.paddle_w as f32;
            let py = self.paddle_y as f32;
            let ph = self.paddle_h as f32;

            if self.ball_vy > 0.0
                && self.ball_y + self.ball_radius >= py
                && self.ball_y - self.ball_radius <= py + ph
                && self.ball_x + self.ball_radius >= px
                && self.ball_x - self.ball_radius <= px + pw
            {
                self.ball_y = py - self.ball_radius;
                self.ball_vy = -self.ball_vy.abs();

                // Directional deflection from impact position + paddle velocity
                let offset = ((self.ball_x - (px + pw / 2.0)) / (pw / 2.0)).clamp(-1.0, 1.0);
                self.ball_vx = self.ball_vx * 0.25 + offset * 0.55 + self.paddle_vx * 0.35;

                // Speed normalization: increases slightly as bricks are cleared
                let speed = (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy).sqrt();
                let cleared_factor = (32usize.saturating_sub(self.bricks_remaining)) as f32 * 0.007;
                let target_speed = (0.72 + cleared_factor).clamp(0.72, 0.95);
                if speed > 0.001 {
                    self.ball_vx = (self.ball_vx / speed) * target_speed;
                    self.ball_vy = (self.ball_vy / speed) * target_speed;
                }
                if self.ball_vy > -0.25 {
                    self.ball_vy = -0.25;
                }

                self.combo = 0; // Reset combo on paddle return
                if self.wave_force_enabled {
                    let impact = GaussianImpulse::new(self.ball_x, py, 2.6, 2.0);
                    impact.inject(&mut self.solver.state);
                }
                event = StepEvent::PaddleHit;
            }

            // 5. Breakable Bricks Collision (Point-of-Impact Fracturing & Cavitation Rupture)
            let mut hit_idx = None;
            let mut hit_normal = (0.0f32, 0.0f32);
            let mut contact_pt = (0.0f32, 0.0f32);

            for (idx, b) in self.bricks.iter().enumerate() {
                if !b.is_alive() {
                    continue;
                }

                let x0 = b.x as f32;
                let x1 = (b.x + b.w) as f32;
                let y0 = b.y as f32;
                let y1 = (b.y + b.h) as f32;

                let cx = self.ball_x.clamp(x0, x1);
                let cy = self.ball_y.clamp(y0, y1);
                let dx = self.ball_x - cx;
                let dy = self.ball_y - cy;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq <= self.ball_radius * self.ball_radius {
                    hit_idx = Some(idx);
                    contact_pt = (cx, cy);

                    // Compute rebound normal
                    if dx.abs() > dy.abs() {
                        hit_normal = (if dx > 0.0 { 1.0 } else { -1.0 }, 0.0);
                    } else {
                        hit_normal = (0.0, if dy > 0.0 { 1.0 } else { -1.0 });
                    }
                    break;
                }
            }

            if let Some(idx) = hit_idx {
                let (hits_left, row, b_copy) = {
                    let brick = &mut self.bricks[idx];
                    brick.hits_left = brick.hits_left.saturating_sub(1);
                    brick.apply_contact_fracture(
                        contact_pt.0,
                        contact_pt.1,
                        hit_normal.0,
                        hit_normal.1,
                    );
                    (brick.hits_left, brick.row, *brick)
                };
                self.combo += 1;

                // Reflect ball velocity along normal
                if hit_normal.0 != 0.0 {
                    self.ball_vx = hit_normal.0.abs() * hit_normal.0.signum() * self.ball_vx.abs();
                    self.ball_x = contact_pt.0 + hit_normal.0 * (self.ball_radius + 0.1);
                }
                if hit_normal.1 != 0.0 {
                    self.ball_vy = hit_normal.1.abs() * hit_normal.1.signum() * self.ball_vy.abs();
                    self.ball_y = contact_pt.1 + hit_normal.1 * (self.ball_radius + 0.1);
                }

                let row_mult = (4 - row as u32).max(1);

                if hits_left == 1 {
                    // HIT 1: POINT-OF-IMPACT STRUCTURAL FRACTURE!
                    // Opens semi-permeable fracture channels through the brick on the Yee lattice
                    self.score += 50 * row_mult * self.combo;
                    self.sync_single_brick_to_lattice(&b_copy);

                    if self.wave_force_enabled {
                        let stress_pulse =
                            GaussianImpulse::new(contact_pt.0, contact_pt.1, 2.4, 2.2);
                        stress_pulse.inject(&mut self.solver.state);
                    }
                    event = StepEvent::BrickCracked {
                        brick_idx: idx,
                        row,
                    };
                } else {
                    // HIT 2: PHYSICALLY SHATTERED!
                    self.score += 150 * row_mult * self.combo;
                    self.bricks_remaining = self.bricks_remaining.saturating_sub(1);

                    // Clear boundary & relax material so sound waves and ball pass right through!
                    self.clear_brick_boundary(&b_copy);

                    // Gate BOTH the explosive cavitation shockwave and momentum boost behind wave_force_enabled
                    if self.wave_force_enabled {
                        let blast = GaussianImpulse::new(contact_pt.0, contact_pt.1, 3.6, 4.0);
                        blast.inject(&mut self.solver.state);
                        self.ball_vx += hit_normal.0 * 0.22;
                        self.ball_vy += hit_normal.1 * 0.22;
                    }

                    event = StepEvent::BrickBroken {
                        brick_idx: idx,
                        row,
                        remaining: self.bricks_remaining,
                    };
                }
            }

            // 6. Drain Line (Ball lost past paddle)
            if self.ball_y >= h - 2.0 {
                self.lives = self.lives.saturating_sub(1);
                self.combo = 0;

                if self.wave_force_enabled {
                    let drain_impulse = GaussianImpulse::new(self.ball_x, h - 2.0, 4.0, -1.8);
                    drain_impulse.inject(&mut self.solver.state);
                }

                if self.lives > 0 {
                    self.ball_attached = true;
                    self.ball_x = self.paddle_x + self.paddle_w as f32 / 2.0;
                    self.ball_y = self.paddle_y as f32 - self.ball_radius - 0.8;
                    self.ball_vx = 0.0;
                    self.ball_vy = 0.0;
                    event = StepEvent::LifeLost {
                        remaining_lives: self.lives,
                    };
                } else {
                    self.in_play = false;
                    event = StepEvent::GameOver;
                }
                break;
            }

            // 7. Victory condition
            if self.bricks_remaining == 0 {
                self.in_play = false;
                event = StepEvent::Victory;
                if self.wave_force_enabled {
                    let win_impulse = GaussianImpulse::new(w / 2.0, h / 2.0, 5.0, 3.0);
                    win_impulse.inject(&mut self.solver.state);
                }
                break;
            }
        }

        // Dissipate paddle movement velocity
        self.paddle_vx *= 0.85;

        // Ball wake excitation (only when field coupling is active)
        if !self.ball_attached && self.in_play && self.wave_force_enabled {
            let wake = GaussianImpulse::new(self.ball_x, self.ball_y, 1.4, 0.35);
            wake.inject(&mut self.solver.state);
        }

        // Advance Dzog FDTD field simulation
        self.solver.advance_macro();

        // 8. Acoustic Wave-Induced Cleaving (Resonance Destruction!)
        // Constructive interference of shockwaves shatters already-weakened bricks without ball contact.
        if self.wave_force_enabled && self.in_play {
            if let Some(cleave_event) = self.check_acoustic_wave_cleaving() {
                if event == StepEvent::None {
                    event = cleave_event;
                }
            }
        }

        // Hamiltonian energy accounting
        self.field_energy = FieldEnergy::compute(&self.solver.state, &self.solver.material);

        if self.score > self.high_score {
            self.high_score = self.score;
        }

        event
    }

    /// Inspect weakened bricks (`hits_left == 1`) for constructive acoustic pressure spikes
    /// exceeding their material `cleave_pressure` threshold.
    fn check_acoustic_wave_cleaving(&mut self) -> Option<StepEvent> {
        let w = self.width;
        let h = self.height;
        let mut cleaved_indices = Vec::new();

        for (idx, b) in self.bricks.iter().enumerate() {
            if b.hits_left != 1 {
                continue;
            }

            // Ensure the ball is not currently inside the brick's immediate collision halo
            // so normal ball hits are handled by kinematic contact, while distant wave spikes cleave!
            let b_cx = b.x as f32 + b.w as f32 * 0.5;
            let b_cy = b.y as f32 + b.h as f32 * 0.5;
            let dist_ball_sq = (self.ball_x - b_cx).powi(2) + (self.ball_y - b_cy).powi(2);
            if !self.ball_attached && dist_ball_sq < (b.w as f32 * 0.65).powi(2) {
                continue;
            }

            let p_crit = b.material_spec().cleave_pressure;
            let mut max_p = 0.0f32;

            let x0 = b.x.saturating_sub(1);
            let x1 = (b.x + b.w).min(w - 1);
            let y0 = b.y.saturating_sub(1);
            let y1 = (b.y + b.h).min(h - 1);

            for y in y0..=y1 {
                for x in x0..=x1 {
                    let cell_idx = self.solver.state.config.index(x, y);
                    let p_abs = self.solver.state.pressure[cell_idx].abs();
                    if p_abs > max_p {
                        max_p = p_abs;
                    }
                }
            }

            if max_p >= p_crit {
                cleaved_indices.push(idx);
            }
        }

        let mut last_event = None;
        for idx in cleaved_indices {
            let b_copy = {
                let brick = &mut self.bricks[idx];
                brick.hits_left = 0;
                brick.damage = [[1.0; 12]; 4];
                *brick
            };
            self.bricks_remaining = self.bricks_remaining.saturating_sub(1);
            self.acoustic_cleaves += 1;
            self.combo += 1;
            let row_mult = (4 - b_copy.row as u32).max(1);
            self.score += 200 * row_mult * self.combo;

            self.clear_brick_boundary(&b_copy);

            // Secondary resonant cavitation burst (enables chain-reaction domino cleaving)
            let cx = b_copy.x as f32 + b_copy.w as f32 * 0.5;
            let cy = b_copy.y as f32 + b_copy.h as f32 * 0.5;
            let resonant_burst = GaussianImpulse::new(cx, cy, 3.0, 2.8);
            resonant_burst.inject(&mut self.solver.state);

            if self.bricks_remaining == 0 {
                self.in_play = false;
                last_event = Some(StepEvent::Victory);
                break;
            } else {
                last_event = Some(StepEvent::BrickBroken {
                    brick_idx: idx,
                    row: b_copy.row,
                    remaining: self.bricks_remaining,
                });
            }
        }

        last_event
    }

    /// Rasterize the 2D acoustic field, breakable bricks, paddle, and ball directly into RGBA8.
    pub fn render_to_pixel_buffer(&mut self) {
        let w = self.width;
        let h = self.height;

        let bx = self.ball_x.round() as isize;
        let by = self.ball_y.round() as isize;
        let br = (self.ball_radius + 0.5) as isize;

        let px_start = self.paddle_x.round() as isize;
        let px_end = px_start + self.paddle_w as isize;
        let py_start = self.paddle_y as isize;
        let py_end = py_start + self.paddle_h as isize;

        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let px_idx = idx * 4;

                let is_wall = x <= 1 || x >= w - 2 || y <= 1;
                let is_drain = y >= h - 2;
                let is_paddle = (x as isize) >= px_start
                    && (x as isize) < px_end
                    && (y as isize) >= py_start
                    && (y as isize) < py_end;

                // Ball test
                let dx = x as isize - bx;
                let dy = y as isize - by;
                let dist_sq = dx * dx + dy * dy;
                let is_ball_core = dist_sq <= br * br;
                let is_ball_halo = dist_sq <= (br + 1) * (br + 1);

                if is_ball_core {
                    // Phosphor white/yellow core
                    self.pixel_buffer[px_idx] = 255;
                    self.pixel_buffer[px_idx + 1] = 255;
                    self.pixel_buffer[px_idx + 2] = 220;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_ball_halo {
                    // Phosphor golden aura
                    self.pixel_buffer[px_idx] = 255;
                    self.pixel_buffer[px_idx + 1] = 205;
                    self.pixel_buffer[px_idx + 2] = 80;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_paddle {
                    // Laser cyan paddle with highlight core
                    let is_paddle_core = (y as isize) == py_start + 1;
                    if is_paddle_core {
                        self.pixel_buffer[px_idx] = 220;
                        self.pixel_buffer[px_idx + 1] = 255;
                        self.pixel_buffer[px_idx + 2] = 255;
                    } else {
                        self.pixel_buffer[px_idx] = 30;
                        self.pixel_buffer[px_idx + 1] = 220;
                        self.pixel_buffer[px_idx + 2] = 250;
                    }
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if let Some(brick) = self.find_brick_at(x, y) {
                    let p_local = self.solver.state.pressure[idx];
                    let (r, g, b) = match self.view_mode {
                        FieldViewMode::ImpedanceAndCohesion => {
                            let z = self.solver.material.impedance_at(x, y);
                            let c = self.solver.material.cohesion[idx].clamp(0.0, 1.0);
                            let r = (z * 95.0).clamp(40.0, 255.0) as u8;
                            let g = (c * 210.0) as u8;
                            let b_col = ((1.0 - c) * 250.0) as u8;
                            (r, g, b_col)
                        }
                        _ => Self::brick_pixel_color(brick, x, y, p_local),
                    };
                    self.pixel_buffer[px_idx] = r;
                    self.pixel_buffer[px_idx + 1] = g;
                    self.pixel_buffer[px_idx + 2] = b;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_wall {
                    // Outer steel border
                    let is_inner_edge = x == 1 || x == w - 2 || y == 1;
                    if is_inner_edge {
                        self.pixel_buffer[px_idx] = 95;
                        self.pixel_buffer[px_idx + 1] = 175;
                        self.pixel_buffer[px_idx + 2] = 240;
                    } else {
                        self.pixel_buffer[px_idx] = 50;
                        self.pixel_buffer[px_idx + 1] = 90;
                        self.pixel_buffer[px_idx + 2] = 145;
                    }
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_drain {
                    // Drain hazard line: alternating warning hashes
                    if (x / 4) % 2 == 0 {
                        self.pixel_buffer[px_idx] = 120;
                        self.pixel_buffer[px_idx + 1] = 25;
                        self.pixel_buffer[px_idx + 2] = 35;
                    } else {
                        self.pixel_buffer[px_idx] = 25;
                        self.pixel_buffer[px_idx + 1] = 10;
                        self.pixel_buffer[px_idx + 2] = 15;
                    }
                    self.pixel_buffer[px_idx + 3] = 255;
                } else {
                    let (r, g, b) = self.field_pixel_color(x, y, idx);
                    self.pixel_buffer[px_idx] = r;
                    self.pixel_buffer[px_idx + 1] = g;
                    self.pixel_buffer[px_idx + 2] = b;
                    self.pixel_buffer[px_idx + 3] = 255;
                }
            }
        }
    }

    /// Map evolving field quantities at `(x, y)` to RGB according to the active `FieldViewMode`.
    fn field_pixel_color(&self, x: usize, y: usize, idx: usize) -> (u8, u8, u8) {
        let p = self.solver.state.pressure[idx];
        let vx = self.solver.state.vx[idx];
        let vy = self.solver.state.vy[idx];
        let v_mag = (vx * vx + vy * vy).sqrt();

        // Highlight user-painted acoustic reflector walls
        if self.solver.boundary.wall_east[idx] || self.solver.boundary.wall_north[idx] {
            return (190, 225, 255);
        }

        match self.view_mode {
            FieldViewMode::CompositeArcade => {
                if p.abs() > 0.015 || v_mag > 0.015 {
                    let (mut r, mut g, mut b) = if p > 0.0 {
                        let t = (p * 1.3).clamp(0.0, 1.0);
                        (
                            (12.0 + t * 240.0) as u8,
                            (14.0 + t * 165.0) as u8,
                            (22.0 + t * 45.0) as u8,
                        )
                    } else {
                        let t = (-p * 1.3).clamp(0.0, 1.0);
                        (
                            (10.0 + t * 25.0) as u8,
                            (16.0 + t * 180.0) as u8,
                            (24.0 + t * 230.0) as u8,
                        )
                    };

                    if v_mag > 0.03 {
                        let vt = (v_mag * 0.7).clamp(0.0, 0.4);
                        r = r.saturating_add((vt * 70.0) as u8);
                        g = g.saturating_add((vt * 110.0) as u8);
                        b = b.saturating_add((vt * 170.0) as u8);
                    }
                    (r, g, b)
                } else {
                    (12, 14, 22)
                }
            }
            FieldViewMode::PressureAndBohmian => {
                let q = self.solver.state.quantum_potential_at(x, y);
                let p_t = (p * 1.4).clamp(-1.0, 1.0);
                let q_t = (q * 0.35).clamp(-1.0, 1.0);

                let r = (18.0 + p_t.max(0.0) * 210.0 + (-q_t).max(0.0) * 160.0).clamp(0.0, 255.0)
                    as u8;
                let g = (18.0 + q_t.max(0.0) * 200.0 + p_t.abs() * 60.0).clamp(0.0, 255.0) as u8;
                let b = (28.0 + (-p_t).max(0.0) * 215.0 + (-q_t).max(0.0) * 190.0)
                    .clamp(0.0, 255.0) as u8;
                (r, g, b)
            }
            FieldViewMode::VelocityAndFlux => {
                let (ix, iy) = self.solver.state.energy_flux_at(x, y);
                let flux_mag = (ix * ix + iy * iy).sqrt();
                let vt = (v_mag * 2.2).clamp(0.0, 1.0);
                let ft = (flux_mag * 2.5).clamp(0.0, 1.0);

                let r = (10.0 + ft * 240.0) as u8;
                let g = (16.0 + vt * 225.0) as u8;
                let b = (24.0 + (vt * 140.0 + ft * 90.0).min(230.0)) as u8;
                (r, g, b)
            }
            FieldViewMode::ImpedanceAndCohesion => {
                let z = self.solver.material.impedance_at(x, y);
                let gamma = self.solver.material.damping[idx];
                let z_norm = ((z - 0.5) / 1.8).clamp(0.0, 1.0);
                let r = (16.0 + z_norm * 160.0 + p.abs() * 70.0).clamp(0.0, 255.0) as u8;
                let g = (22.0 + (1.0 - gamma * 12.0).clamp(0.0, 1.0) * 65.0) as u8;
                let b = (38.0 + z_norm * 190.0) as u8;
                (r, g, b)
            }
        }
    }

    #[inline]
    fn find_brick_at(&self, x: usize, y: usize) -> Option<&Brick> {
        self.bricks
            .iter()
            .find(|b| b.is_alive() && x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h)
    }

    /// Color and dynamic point-of-impact fracture fissure computation for a brick pixel.
    fn brick_pixel_color(b: &Brick, x: usize, y: usize, p_local: f32) -> (u8, u8, u8) {
        let (base_r, base_g, base_b): (u8, u8, u8) = match b.row {
            0 => (235, 45, 75),  // Row 0: Crystal Quartz (Ruby)
            1 => (245, 160, 25), // Row 1: Basalt Anvil (Solar Gold)
            2 => (35, 215, 110), // Row 2: Viscous Lens (Emerald)
            _ => (0, 210, 245),  // Row 3: Sonic Resonator (Electric Cyan)
        };

        let is_top_edge = y == b.y;
        let is_left_edge = x == b.x;
        let is_bot_edge = y == b.y + b.h - 1;
        let is_right_edge = x == b.x + b.w - 1;

        let rel_x = (x - b.x).min(11);
        let rel_y = (y - b.y).min(3);
        let cell_dmg = b.damage[rel_y][rel_x];

        if b.hits_left == 2 && cell_dmg < 0.2 {
            // Intact brick: bright 3D bevel edges + subtle acoustic shimmer when waves hit it
            let wave_glow = (p_local.abs() * 35.0).clamp(0.0, 40.0) as u8;
            if is_top_edge || is_left_edge {
                (
                    base_r.saturating_add(45).saturating_add(wave_glow),
                    base_g.saturating_add(45).saturating_add(wave_glow),
                    base_b.saturating_add(45).saturating_add(wave_glow),
                )
            } else if is_bot_edge || is_right_edge {
                (
                    base_r.saturating_sub(55).saturating_add(wave_glow),
                    base_g.saturating_sub(55).saturating_add(wave_glow),
                    base_b.saturating_sub(55).saturating_add(wave_glow),
                )
            } else {
                (
                    base_r.saturating_add(wave_glow),
                    base_g.saturating_add(wave_glow),
                    base_b.saturating_add(wave_glow),
                )
            }
        } else {
            // Point-of-Impact Fractured Brick!
            // Edge cells near the impact crater chip away into dark void
            let is_edge = is_top_edge || is_left_edge || is_bot_edge || is_right_edge;
            if is_edge && cell_dmg >= 0.85 {
                // Chipped-away spall crater on the struck edge
                return (18, 22, 34);
            }

            if cell_dmg >= 0.60 {
                // Incandescent fracture fissure vein radiating from contact point (pulses with acoustic pressure)
                let pulse = (p_local.abs() * 60.0).clamp(0.0, 55.0) as u8;
                (255, 225u8.saturating_add(pulse / 2), 140u8.saturating_add(pulse))
            } else {
                // Stressed, darkened body around the fracture veins
                let stress_factor = 0.65 - 0.25 * cell_dmg;
                (
                    (base_r as f32 * stress_factor) as u8,
                    (base_g as f32 * stress_factor * 0.92) as u8,
                    (base_b as f32 * stress_factor * 0.85) as u8,
                )
            }
        }
    }
}
