//! Field-First Pong: Cohesive ball entity coupled to Dzog acoustic lattice.
//!
//! Solves the 2D dispersion issue:
//! - Ball maintains directed kinematics (position, velocity, angle deflection).
//! - Every motion, wall bounce, and paddle strike excites authentic acoustic
//!   shockwaves on the underlying Dzog staggered FDTD grid.
//! - The live acoustic lattice provides dynamic wave ripple aesthetics,
//!   cavity resonances, and impact feedback.
//! - Goals are scored when the ball physically crosses the goal line.

use dzog_core::*;

pub struct SolitonFieldGame {
    pub solver: AcousticSolver,
    pub width: usize,
    pub height: usize,

    // Ball kinematics
    pub ball_x: f32,
    pub ball_y: f32,
    pub ball_vx: f32,
    pub ball_vy: f32,
    pub ball_radius: f32,

    // Paddle boundary coordinates on the lattice
    pub p1_paddle_y: usize,
    pub p2_paddle_y: usize,
    pub paddle_height: usize,
    pub p1_vy: f32,
    pub p2_vy: f32,

    // Scoring & State
    pub p1_score: u32,
    pub p2_score: u32,
    pub in_play: bool,
    pub rally_count: u32,

    // Pixel framebuffer for GUI rendering (RGBA8)
    pub pixel_buffer: Vec<u8>,
}

impl SolitonFieldGame {
    pub fn new(width: usize, height: usize) -> Self {
        let grid = GridConfig::new(width, height, 1.0, 1.0);
        let state = AcousticState::new(grid);

        // Acoustic medium with slight damping to keep fluid responsive
        let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.008);
        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);

        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        let nl = NonlinearLaw {
            beta: 0.6,
            alpha: 0.4,
            threshold_decay: 0.02,
        };
        let config = SolverConfig::new(dt, 0.7, 4).with_nonlinearity(nl);

        let solver = AcousticSolver::new(state, material, boundary, config).unwrap();

        let paddle_height = (height / 4).max(8);
        let pixel_count = width * height * 4;

        let mut game = Self {
            solver,
            width,
            height,
            ball_x: width as f32 / 2.0,
            ball_y: height as f32 / 2.0,
            ball_vx: 0.0,
            ball_vy: 0.0,
            ball_radius: 1.5,
            p1_paddle_y: height / 2 - paddle_height / 2,
            p2_paddle_y: height / 2 - paddle_height / 2,
            paddle_height,
            p1_vy: 0.0,
            p2_vy: 0.0,
            p1_score: 0,
            p2_score: 0,
            in_play: false,
            rally_count: 0,
            pixel_buffer: vec![0; pixel_count],
        };

        game.sync_paddle_boundaries();
        game
    }

    /// Update moving boundary face conditions on the lattice.
    pub fn sync_paddle_boundaries(&mut self) {
        let w = self.width;
        let h = self.height;

        self.solver.boundary.clear_interior();

        // Paddle 1: placed at column x = 6
        let p1_x = 6;
        let p1_y_end = (self.p1_paddle_y + self.paddle_height).min(h - 1);
        for y in self.p1_paddle_y..=p1_y_end {
            let idx = self.solver.state.config.index(p1_x, y);
            self.solver.boundary.wall_east[idx] = true;
            self.solver.boundary.wall_vx[idx] = 0.0;
            self.solver.boundary.wall_vy[idx] = self.p1_vy;
        }

        // Paddle 2: placed at column x = w - 8
        let p2_x = w - 8;
        let p2_y_end = (self.p2_paddle_y + self.paddle_height).min(h - 1);
        for y in self.p2_paddle_y..=p2_y_end {
            let idx = self.solver.state.config.index(p2_x, y);
            self.solver.boundary.wall_east[idx] = true;
            self.solver.boundary.wall_vx[idx] = 0.0;
            self.solver.boundary.wall_vy[idx] = self.p2_vy;
        }
    }

    /// Move player 1 paddle up or down.
    pub fn move_p1(&mut self, dy: isize) {
        let max_y = self.height.saturating_sub(self.paddle_height + 1);
        let new_y = (self.p1_paddle_y as isize + dy).clamp(1, max_y as isize) as usize;
        self.p1_vy = (dy as f32) * 0.5;
        self.p1_paddle_y = new_y;
        self.sync_paddle_boundaries();
    }

    /// Move player 2 paddle up or down.
    pub fn move_p2(&mut self, dy: isize) {
        let max_y = self.height.saturating_sub(self.paddle_height + 1);
        let new_y = (self.p2_paddle_y as isize + dy).clamp(1, max_y as isize) as usize;
        self.p2_vy = (dy as f32) * 0.5;
        self.p2_paddle_y = new_y;
        self.sync_paddle_boundaries();
    }

    /// Serve a new ball from court center towards player 1 or player 2.
    pub fn serve(&mut self, towards_player: u8) {
        self.solver.state.clear();
        self.ball_x = self.width as f32 / 2.0;
        self.ball_y = self.height as f32 / 2.0;

        let speed = 0.65f32;
        let angle = 0.22f32;

        if towards_player == 1 {
            self.ball_vx = -speed * angle.cos();
            self.ball_vy = speed * angle.sin();
        } else {
            self.ball_vx = speed * angle.cos();
            self.ball_vy = -speed * angle.sin();
        }

        self.in_play = true;
        self.rally_count = 0;

        // Inject initial acoustic serve wake
        let impulse = GaussianImpulse::new(self.ball_x, self.ball_y, 2.5, 1.2);
        impulse.inject(&mut self.solver.state);
    }

    /// Advance game physics and Dzog field substeps.
    pub fn step(&mut self) -> Option<u8> {
        if !self.in_play {
            return None;
        }

        let w = self.width as f32;
        let h = self.height as f32;

        // Kinematics substeps for continuous collision detection
        let sub_steps = 4;
        let dt_sub = 1.0 / sub_steps as f32;
        let mut scorer = None;

        for _ in 0..sub_steps {
            self.ball_x += self.ball_vx * dt_sub;
            self.ball_y += self.ball_vy * dt_sub;

            // 1. Top and Bottom Wall collisions
            if self.ball_y - self.ball_radius <= 1.0 {
                self.ball_y = 1.0 + self.ball_radius;
                self.ball_vy = self.ball_vy.abs();
                // Excite acoustic shockwave on wall bounce
                let impact = GaussianImpulse::new(self.ball_x, 1.0, 2.0, 1.4);
                impact.inject(&mut self.solver.state);
            } else if self.ball_y + self.ball_radius >= h - 2.0 {
                self.ball_y = h - 2.0 - self.ball_radius;
                self.ball_vy = -self.ball_vy.abs();
                let impact = GaussianImpulse::new(self.ball_x, h - 2.0, 2.0, 1.4);
                impact.inject(&mut self.solver.state);
            }

            // 2. Paddle 1 (Left) collision at column x = 6
            let p1_front = 7.0;
            let p1_top = self.p1_paddle_y as f32;
            let p1_bot = p1_top + self.paddle_height as f32;

            if self.ball_vx < 0.0
                && self.ball_x - self.ball_radius <= p1_front
                && self.ball_x + self.ball_radius >= 5.0
                && self.ball_y >= p1_top - self.ball_radius
                && self.ball_y <= p1_bot + self.ball_radius
            {
                self.ball_x = p1_front + self.ball_radius;
                self.ball_vx = self.ball_vx.abs() * 1.02; // Slight acceleration per rally
                let offset = (self.ball_y - (p1_top + p1_bot) / 2.0) / (self.paddle_height as f32 / 2.0);
                self.ball_vy = self.ball_vy * 0.4 + offset * 0.5 + self.p1_vy * 0.35;
                self.rally_count += 1;

                // Inject acoustic impact shockwave
                let impact = GaussianImpulse::new(p1_front, self.ball_y, 2.8, 1.8);
                impact.inject(&mut self.solver.state);
            }

            // 3. Paddle 2 (Right) collision at column x = w - 8
            let p2_front = w - 8.0;
            let p2_top = self.p2_paddle_y as f32;
            let p2_bot = p2_top + self.paddle_height as f32;

            if self.ball_vx > 0.0
                && self.ball_x + self.ball_radius >= p2_front
                && self.ball_x - self.ball_radius <= w - 6.0
                && self.ball_y >= p2_top - self.ball_radius
                && self.ball_y <= p2_bot + self.ball_radius
            {
                self.ball_x = p2_front - self.ball_radius;
                self.ball_vx = -self.ball_vx.abs() * 1.02;
                let offset = (self.ball_y - (p2_top + p2_bot) / 2.0) / (self.paddle_height as f32 / 2.0);
                self.ball_vy = self.ball_vy * 0.4 + offset * 0.5 + self.p2_vy * 0.35;
                self.rally_count += 1;

                // Inject acoustic impact shockwave
                let impact = GaussianImpulse::new(p2_front, self.ball_y, 2.8, 1.8);
                impact.inject(&mut self.solver.state);
            }

            // 4. Goal Detection
            if self.ball_x < 2.0 {
                // Ball passed player 1 -> Player 2 scores
                self.p2_score += 1;
                self.in_play = false;
                scorer = Some(2);
                break;
            } else if self.ball_x > w - 3.0 {
                // Ball passed player 2 -> Player 1 scores
                self.p1_score += 1;
                self.in_play = false;
                scorer = Some(1);
                break;
            }
        }

        // Couple ball continuous motion to field as an acoustic wake
        if self.in_play {
            let wake = GaussianImpulse::new(self.ball_x, self.ball_y, 1.5, 0.4);
            wake.inject(&mut self.solver.state);
        }

        // Advance Dzog FDTD field simulation
        self.solver.advance_macro();

        scorer
    }

    /// Rasterize the 2D field state and game elements directly into an RGBA8 pixel buffer.
    pub fn render_to_pixel_buffer(&mut self) {
        let w = self.width;
        let h = self.height;

        let bx = self.ball_x.round() as isize;
        let by = self.ball_y.round() as isize;
        let br = (self.ball_radius + 0.5) as isize;

        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let px_idx = idx * 4;

                let is_p1 = x >= 5 && x <= 6 && y >= self.p1_paddle_y && y < self.p1_paddle_y + self.paddle_height;
                let is_p2 = x >= w - 8 && x <= w - 7 && y >= self.p2_paddle_y && y < self.p2_paddle_y + self.paddle_height;
                let is_wall = y == 0 || y == h - 1;
                let is_net = x == w / 2 && y % 3 == 0;

                // Ball entity test (within radius)
                let dx = x as isize - bx;
                let dy = y as isize - by;
                let is_ball = self.in_play && (dx * dx + dy * dy <= br * br);

                if is_ball {
                    // Bright phosphor white-yellow core
                    self.pixel_buffer[px_idx] = 255;
                    self.pixel_buffer[px_idx + 1] = 255;
                    self.pixel_buffer[px_idx + 2] = 210;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_p1 || is_p2 {
                    // Crisp phosphor cyan paddles
                    self.pixel_buffer[px_idx] = 80;
                    self.pixel_buffer[px_idx + 1] = 230;
                    self.pixel_buffer[px_idx + 2] = 255;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_wall {
                    // Boundary borders
                    self.pixel_buffer[px_idx] = 160;
                    self.pixel_buffer[px_idx + 1] = 170;
                    self.pixel_buffer[px_idx + 2] = 200;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_net {
                    // Dotted center court net
                    self.pixel_buffer[px_idx] = 50;
                    self.pixel_buffer[px_idx + 1] = 70;
                    self.pixel_buffer[px_idx + 2] = 95;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else {
                    // Sample living Dzog acoustic wave field for background ripples
                    let p = self.solver.state.pressure[idx];
                    let vx = self.solver.state.vx[idx];
                    let vy = self.solver.state.vy[idx];
                    let v_mag = (vx * vx + vy * vy).sqrt();

                    if p.abs() > 0.02 || v_mag > 0.02 {
                        if p > 0.0 {
                            // Positive acoustic compression: warm amber shockwave glow
                            let t = (p * 1.2).clamp(0.0, 1.0);
                            let r = (20.0 + t * 215.0) as u8;
                            let g = (15.0 + t * 140.0) as u8;
                            let b = (35.0 + t * 50.0) as u8;
                            self.pixel_buffer[px_idx] = r;
                            self.pixel_buffer[px_idx + 1] = g;
                            self.pixel_buffer[px_idx + 2] = b;
                        } else {
                            // Negative acoustic rarefaction: cool electric cyan ripple
                            let t = (-p * 1.2).clamp(0.0, 1.0);
                            let r = (10.0 + t * 30.0) as u8;
                            let g = (20.0 + t * 160.0) as u8;
                            let b = (45.0 + t * 195.0) as u8;
                            self.pixel_buffer[px_idx] = r;
                            self.pixel_buffer[px_idx + 1] = g;
                            self.pixel_buffer[px_idx + 2] = b;
                        }
                    } else {
                        // Deep retro CRT slate background
                        self.pixel_buffer[px_idx] = 10;
                        self.pixel_buffer[px_idx + 1] = 12;
                        self.pixel_buffer[px_idx + 2] = 20;
                    }
                    self.pixel_buffer[px_idx + 3] = 255;
                }
            }
        }
    }
}
