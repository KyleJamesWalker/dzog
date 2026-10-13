//! Field-First Breakout Pong: Single-player Breakout on the Dzog acoustic lattice.
//!
//! Demonstrates Dzoglang's core philosophy:
//! - Paddles and Bricks are physical acoustic boundaries and impedance interfaces on the Yee lattice.
//! - Bricks withstand 2 hits:
//!   - 1st hit: Mechanical stress fracture. Injects acoustic stress pulse, shifts color and shows fracture veins.
//!   - 2nd hit: Cavitation rupture! Injects explosive sonic burst into the lattice, boundary is cleared,
//!     opening space for sound waves and the ball to pass through.
//! - Acoustic Radiation Force: Sonic blast pressure gradients -grad(p) dynamically push the ball.
//! - Real-time Hamiltonian field energy accounting (potential, kinetic, total).
//! - Retro CRT RGBA8 pixel rasterization with scanlines, breakable brick art, and wave ripple optics.

use dzog_core::*;

/// A breakable brick on the acoustic lattice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Brick {
    /// Left column on the grid.
    pub x: usize,
    /// Top row on the grid.
    pub y: usize,
    /// Width in cells.
    pub w: usize,
    /// Height in cells.
    pub h: usize,
    /// Hits remaining before physical destruction (2 = solid, 1 = cracked, 0 = destroyed).
    pub hits_left: u8,
    /// Row index (0..3) determining color palette and point value.
    pub row: u8,
}

impl Brick {
    pub fn new(x: usize, y: usize, w: usize, h: usize, row: u8) -> Self {
        Self {
            x,
            y,
            w,
            h,
            hits_left: 2,
            row,
        }
    }

    /// Whether this brick is intact and physically blocking.
    #[inline]
    pub fn is_alive(&self) -> bool {
        self.hits_left > 0
    }
}

/// Simulation event emitted during a physics step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepEvent {
    None,
    /// Brick was struck for the 1st time, causing stress cracks.
    BrickCracked { brick_idx: usize, row: u8 },
    /// Brick was struck for the 2nd time, physically shattering.
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

    // Engine mechanics
    pub wave_force_enabled: bool,
    pub field_energy: FieldEnergy,

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

        // Acoustic medium with slight damping to keep waves lively but bounded
        let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.007);
        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);

        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
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
            field_energy: FieldEnergy { potential: 0.0, kinetic: 0.0, total: 0.0 },
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
        self.init_bricks();
        self.sync_all_brick_boundaries();

        self.lives = 3;
        self.score = 0;
        self.combo = 0;
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

    /// Enforce rigid acoustic boundary conditions for all living bricks on the lattice.
    pub fn sync_all_brick_boundaries(&mut self) {
        let w = self.width;
        let h = self.height;

        for b in &self.bricks {
            if b.is_alive() {
                let x_min = b.x.saturating_sub(1);
                let x_max = (b.x + b.w - 1).min(w - 1);
                let y_min = b.y.saturating_sub(1);
                let y_max = (b.y + b.h - 1).min(h - 1);
                self.solver.boundary.set_moving_rect(x_min, x_max, y_min, y_max, 0.0, 0.0);
            }
        }
    }

    /// Clear a specific brick's boundary from the lattice when destroyed.
    pub fn clear_brick_boundary(&mut self, b: &Brick) {
        let w = self.width;
        let h = self.height;
        let x_min = b.x.saturating_sub(1);
        let x_max = (b.x + b.w - 1).min(w - 1);
        let y_min = b.y.saturating_sub(1);
        let y_max = (b.y + b.h - 1).min(h - 1);
        self.solver.boundary.clear_region(x_min, x_max, y_min, y_max);
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

        self.solver.boundary.set_moving_rect(px0, px1, py0, py1, self.paddle_vx, 0.0);
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

        // Excite acoustic launch wake
        let impulse = GaussianImpulse::new(self.ball_x, self.ball_y, 2.4, 1.4);
        impulse.inject(&mut self.solver.state);
    }

    /// Toggle acoustic radiation pressure coupling on/off.
    pub fn toggle_wave_force(&mut self) {
        self.wave_force_enabled = !self.wave_force_enabled;
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

            // 1. Acoustic radiation pressure gradient force coupling:
            // The ball is physically accelerated by -grad(p) from acoustic shockwaves and sonic bursts!
            if self.wave_force_enabled {
                let bx_round = self.ball_x.round() as isize;
                let by_round = self.ball_y.round() as isize;
                if bx_round >= 1
                    && bx_round < (self.width as isize - 1)
                    && by_round >= 1
                    && by_round < (self.height as isize - 1)
                {
                    let x = bx_round as usize;
                    let y = by_round as usize;
                    let p_east = self.solver.state.pressure[self.solver.state.config.index(x + 1, y)];
                    let p_west = self.solver.state.pressure[self.solver.state.config.index(x - 1, y)];
                    let p_north = self.solver.state.pressure[self.solver.state.config.index(x, y + 1)];
                    let p_south = self.solver.state.pressure[self.solver.state.config.index(x, y - 1)];

                    let grad_x = (p_east - p_west) * 0.5;
                    let grad_y = (p_north - p_south) * 0.5;

                    let k_acoustic = 0.035;
                    self.ball_vx -= grad_x * k_acoustic * dt_sub;
                    self.ball_vy -= grad_y * k_acoustic * dt_sub;
                }
            }

            // 2. Continuous kinematic step
            self.ball_x += self.ball_vx * dt_sub;
            self.ball_y += self.ball_vy * dt_sub;

            // 3. Wall collisions (Left, Right, Top)
            if self.ball_x - self.ball_radius <= 2.0 {
                self.ball_x = 2.0 + self.ball_radius;
                self.ball_vx = self.ball_vx.abs();
                let impact = GaussianImpulse::new(2.0, self.ball_y, 2.0, 1.2);
                impact.inject(&mut self.solver.state);
                if event == StepEvent::None {
                    event = StepEvent::WallHit;
                }
            } else if self.ball_x + self.ball_radius >= w - 3.0 {
                self.ball_x = w - 3.0 - self.ball_radius;
                self.ball_vx = -self.ball_vx.abs();
                let impact = GaussianImpulse::new(w - 3.0, self.ball_y, 2.0, 1.2);
                impact.inject(&mut self.solver.state);
                if event == StepEvent::None {
                    event = StepEvent::WallHit;
                }
            }

            if self.ball_y - self.ball_radius <= 2.0 {
                self.ball_y = 2.0 + self.ball_radius;
                self.ball_vy = self.ball_vy.abs();
                let impact = GaussianImpulse::new(self.ball_x, 2.0, 2.2, 1.4);
                impact.inject(&mut self.solver.state);
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
                let cleared_factor = (32 - self.bricks_remaining) as f32 * 0.007;
                let target_speed = (0.72 + cleared_factor).clamp(0.72, 0.95);
                if speed > 0.001 {
                    self.ball_vx = (self.ball_vx / speed) * target_speed;
                    self.ball_vy = (self.ball_vy / speed) * target_speed;
                }
                if self.ball_vy > -0.25 {
                    self.ball_vy = -0.25;
                }

                self.combo = 0; // Reset combo on paddle return
                let impact = GaussianImpulse::new(self.ball_x, py, 2.6, 2.0);
                impact.inject(&mut self.solver.state);
                event = StepEvent::PaddleHit;
            }

            // 5. Breakable Bricks Collision (2-Hit Mechanics!)
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
                    // HIT 1: CRACKED!
                    // Injects an acoustic fracture pulse into the Yee lattice
                    self.score += 50 * row_mult * self.combo;
                    let stress_pulse = GaussianImpulse::new(contact_pt.0, contact_pt.1, 2.4, 2.2);
                    stress_pulse.inject(&mut self.solver.state);
                    event = StepEvent::BrickCracked {
                        brick_idx: idx,
                        row,
                    };
                } else {
                    // HIT 2: PHYSICALLY SHATTERED!
                    // Removes brick boundary from Yee lattice + Acoustic Cavitation Rupture
                    self.score += 150 * row_mult * self.combo;
                    self.bricks_remaining = self.bricks_remaining.saturating_sub(1);

                    // Clear boundary so sound waves and ball pass right through!
                    self.clear_brick_boundary(&b_copy);

                    // Massive acoustic blast shockwave
                    let blast = GaussianImpulse::new(contact_pt.0, contact_pt.1, 3.6, 4.0);
                    blast.inject(&mut self.solver.state);

                    // Sonic blast force pushes the ball away from the cavitation center
                    if self.wave_force_enabled {
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

                // Low-frequency drain shockwave
                let drain_impulse = GaussianImpulse::new(self.ball_x, h - 2.0, 4.0, -1.8);
                drain_impulse.inject(&mut self.solver.state);

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
                let win_impulse = GaussianImpulse::new(w / 2.0, h / 2.0, 5.0, 3.0);
                win_impulse.inject(&mut self.solver.state);
                break;
            }
        }

        // Dissipate paddle movement velocity
        self.paddle_vx *= 0.85;

        // Ball wake excitation
        if !self.ball_attached && self.in_play {
            let wake = GaussianImpulse::new(self.ball_x, self.ball_y, 1.4, 0.35);
            wake.inject(&mut self.solver.state);
        }

        // Advance Dzog FDTD field simulation
        self.solver.advance_macro();

        // Hamiltonian energy accounting
        self.field_energy = FieldEnergy::compute(&self.solver.state, &self.solver.material);

        if self.score > self.high_score {
            self.high_score = self.score;
        }

        event
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
                    // Breakable brick rasterization
                    let (r, g, b) = Self::brick_pixel_color(brick, x, y);
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
                    // Live Dzog acoustic field wave visualization
                    let p = self.solver.state.pressure[idx];
                    let vx = self.solver.state.vx[idx];
                    let vy = self.solver.state.vy[idx];
                    let v_mag = (vx * vx + vy * vy).sqrt();

                    if p.abs() > 0.015 || v_mag > 0.015 {
                        if p > 0.0 {
                            // Compression: warm amber shockwave glow
                            let t = (p * 1.3).clamp(0.0, 1.0);
                            let r = (12.0 + t * 240.0) as u8;
                            let g = (14.0 + t * 165.0) as u8;
                            let b = (22.0 + t * 45.0) as u8;
                            self.pixel_buffer[px_idx] = r;
                            self.pixel_buffer[px_idx + 1] = g;
                            self.pixel_buffer[px_idx + 2] = b;
                        } else {
                            // Rarefaction: cool electric cyan ripple
                            let t = (-p * 1.3).clamp(0.0, 1.0);
                            let r = (10.0 + t * 25.0) as u8;
                            let g = (16.0 + t * 180.0) as u8;
                            let b = (24.0 + t * 230.0) as u8;
                            self.pixel_buffer[px_idx] = r;
                            self.pixel_buffer[px_idx + 1] = g;
                            self.pixel_buffer[px_idx + 2] = b;
                        }

                        // Add kinetic velocity shimmer
                        if v_mag > 0.03 {
                            let vt = (v_mag * 0.7).clamp(0.0, 0.4);
                            self.pixel_buffer[px_idx] = self.pixel_buffer[px_idx]
                                .saturating_add((vt * 70.0) as u8);
                            self.pixel_buffer[px_idx + 1] = self.pixel_buffer[px_idx + 1]
                                .saturating_add((vt * 110.0) as u8);
                            self.pixel_buffer[px_idx + 2] = self.pixel_buffer[px_idx + 2]
                                .saturating_add((vt * 170.0) as u8);
                        }
                    } else {
                        // Deep retro CRT slate
                        self.pixel_buffer[px_idx] = 12;
                        self.pixel_buffer[px_idx + 1] = 14;
                        self.pixel_buffer[px_idx + 2] = 22;
                    }
                    self.pixel_buffer[px_idx + 3] = 255;
                }
            }
        }
    }

    #[inline]
    fn find_brick_at(&self, x: usize, y: usize) -> Option<&Brick> {
        self.bricks.iter().find(|b| {
            b.is_alive() && x >= b.x && x < b.x + b.w && y >= b.y && y < b.y + b.h
        })
    }

    /// Color and crack texture computation for a brick pixel.
    fn brick_pixel_color(b: &Brick, x: usize, y: usize) -> (u8, u8, u8) {
        let (base_r, base_g, base_b): (u8, u8, u8) = match b.row {
            0 => (235, 45, 75),  // Ruby Red
            1 => (245, 160, 25), // Solar Gold
            2 => (35, 215, 110), // Emerald Green
            _ => (0, 210, 245),  // Electric Cyan
        };

        let is_top_edge = y == b.y;
        let is_left_edge = x == b.x;
        let is_bot_edge = y == b.y + b.h - 1;
        let is_right_edge = x == b.x + b.w - 1;

        if b.hits_left == 2 {
            // Intact brick: bright 3D bevel edges
            if is_top_edge || is_left_edge {
                (
                    base_r.saturating_add(45),
                    base_g.saturating_add(45),
                    base_b.saturating_add(45),
                )
            } else if is_bot_edge || is_right_edge {
                (
                    base_r.saturating_sub(55),
                    base_g.saturating_sub(55),
                    base_b.saturating_sub(55),
                )
            } else {
                (base_r, base_g, base_b)
            }
        } else {
            // Hit 1: CRACKED / STRESSED BRICK!
            // Shows fracture fissure veins running through the brick body
            let rel_x = x - b.x;
            let rel_y = y - b.y;

            let is_crack = (rel_x == rel_y * 3 + 1)
                || (rel_x == rel_y * 3 + 2)
                || (rel_x + rel_y == 8)
                || (rel_y == 2 && rel_x >= 4 && rel_x <= 8);

            if is_crack {
                // Incandescent stress fissure vein
                (255, 240, 170)
            } else {
                // Stressed, darkened body with amber undertone
                (
                    (base_r as f32 * 0.65) as u8,
                    (base_g as f32 * 0.60) as u8,
                    (base_b as f32 * 0.50) as u8,
                )
            }
        }
    }
}
