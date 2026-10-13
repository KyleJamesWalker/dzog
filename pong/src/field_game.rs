//! Field-First Soliton Pong: 100% simulated on the Dzog acoustic lattice.
//!
//! No rigid-body objects or collision boxes:
//! - Ball is a coherent solitary wave packet (soliton) maintained by nonlinear self-focusing.
//! - Paddles are moving boundary conditions (vn = v_wall) in the lattice mask.
//! - Goals are detected via spatial pressure probes behind each paddle.

use dzog_core::*;

pub struct SolitonFieldGame {
    pub solver: AcousticSolver,
    pub width: usize,
    pub height: usize,

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
    pub goal_probe_left: usize,
    pub goal_probe_right: usize,

    // Pixel framebuffer for GUI rendering (RGBA8)
    pub pixel_buffer: Vec<u8>,
}

impl SolitonFieldGame {
    pub fn new(width: usize, height: usize) -> Self {
        let grid = GridConfig::new(width, height, 1.0, 1.0);
        let state = AcousticState::new(grid);

        // Heterogeneous medium: light damping in back goals, lossless in court
        let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0005);
        material.set_region(0, 3, 0, height, 1.0, 1.0, 0.08); // Goal net 1 damping
        material.set_region(width - 4, width, 0, height, 1.0, 1.0, 0.08); // Goal net 2 damping

        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);

        // Configure solver with nonlinear solitary self-focusing law
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        let nl = NonlinearLaw {
            beta: 1.1,             // Self-focusing strength
            alpha: 0.6,            // Saturation preventing amplitude blowup
            threshold_decay: 0.03, // Cleans sub-threshold background noise
        };
        let config = SolverConfig::new(dt, 0.7, 6).with_nonlinearity(nl);

        let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

        // Place goal detection sensor probes behind each paddle
        let probe_left = PressureProbe::new("goal_p1", 2, height / 2, 1).with_threshold(0.20);
        let probe_right = PressureProbe::new("goal_p2", width - 3, height / 2, 1).with_threshold(0.20);

        solver.add_probe(probe_left);
        solver.add_probe(probe_right);

        let paddle_height = (height / 4).max(6);
        let pixel_count = width * height * 4;

        let mut game = Self {
            solver,
            width,
            height,
            p1_paddle_y: height / 2 - paddle_height / 2,
            p2_paddle_y: height / 2 - paddle_height / 2,
            paddle_height,
            p1_vy: 0.0,
            p2_vy: 0.0,
            p1_score: 0,
            p2_score: 0,
            in_play: false,
            goal_probe_left: 0,
            goal_probe_right: 1,
            pixel_buffer: vec![0; pixel_count],
        };

        game.sync_paddle_boundaries();
        game
    }

    /// Update moving boundary face conditions on the lattice.
    pub fn sync_paddle_boundaries(&mut self) {
        let w = self.width;
        let h = self.height;

        // Clear existing interior walls
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
        self.p1_vy = (dy as f32) * 0.45;
        self.p1_paddle_y = new_y;
        self.sync_paddle_boundaries();
    }

    /// Move player 2 paddle up or down.
    pub fn move_p2(&mut self, dy: isize) {
        let max_y = self.height.saturating_sub(self.paddle_height + 1);
        let new_y = (self.p2_paddle_y as isize + dy).clamp(1, max_y as isize) as usize;
        self.p2_vy = (dy as f32) * 0.45;
        self.p2_paddle_y = new_y;
        self.sync_paddle_boundaries();
    }

    /// Serve a new solitary wave packet.
    pub fn serve(&mut self, towards_player: u8) {
        self.solver.state.clear();
        self.solver.probes[self.goal_probe_left].clear();
        self.solver.probes[self.goal_probe_right].clear();

        let cx = self.width as f32 / 2.0;
        let cy = self.height as f32 / 2.0;
        let angle = if towards_player == 1 {
            std::f32::consts::PI + 0.18 // Towards left
        } else {
            -0.18 // Towards right
        };

        // Injected packet amplitude matches the solitary saturation plateau
        let mut packet = DirectionalWavePacket::new(cx, cy, 2.2, 1.4, angle);
        packet.inject(&mut self.solver.state, &self.solver.material);
        self.in_play = true;
    }

    /// Step simulation forward and check goal probes.
    pub fn step(&mut self) -> Option<u8> {
        if !self.in_play {
            return None;
        }

        self.solver.advance_macro();

        // Check acoustic sensor probes
        let left_goal = self.solver.probes[self.goal_probe_left].is_triggered();
        let right_goal = self.solver.probes[self.goal_probe_right].is_triggered();

        if left_goal {
            self.p2_score += 1;
            self.in_play = false;
            Some(2)
        } else if right_goal {
            self.p1_score += 1;
            self.in_play = false;
            Some(1)
        } else {
            None
        }
    }

    /// Find the centroid of the soliton packet in the field.
    pub fn find_soliton_centroid(&self) -> (f32, f32) {
        let w = self.width;
        let h = self.height;
        let mut max_val = 0.0f32;
        let mut best_x = w as f32 / 2.0;
        let mut best_y = h as f32 / 2.0;

        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let p = self.solver.state.pressure[idx].abs();
                if p > max_val {
                    max_val = p;
                    best_x = x as f32;
                    best_y = y as f32;
                }
            }
        }
        (best_x, best_y)
    }

    /// Rasterize the 2D field state directly into an RGBA8 pixel buffer with retro Atari styling.
    pub fn render_to_pixel_buffer(&mut self) {
        let w = self.width;
        let h = self.height;

        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let px_idx = idx * 4;

                let is_p1 = x == 6 && y >= self.p1_paddle_y && y < self.p1_paddle_y + self.paddle_height;
                let is_p2 = x == w - 8 && y >= self.p2_paddle_y && y < self.p2_paddle_y + self.paddle_height;
                let is_wall = y == 0 || y == h - 1;
                let is_net = x == w / 2 && y % 3 == 0;

                if is_p1 || is_p2 {
                    // Bright phosphor cyan paddle
                    self.pixel_buffer[px_idx] = 100;
                    self.pixel_buffer[px_idx + 1] = 240;
                    self.pixel_buffer[px_idx + 2] = 255;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_wall {
                    // Arena boundary
                    self.pixel_buffer[px_idx] = 180;
                    self.pixel_buffer[px_idx + 1] = 180;
                    self.pixel_buffer[px_idx + 2] = 210;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else if is_net {
                    // Center dotted net
                    self.pixel_buffer[px_idx] = 60;
                    self.pixel_buffer[px_idx + 1] = 80;
                    self.pixel_buffer[px_idx + 2] = 110;
                    self.pixel_buffer[px_idx + 3] = 255;
                } else {
                    // Render field pressure perturbation and kinetic energy glow
                    let p = self.solver.state.pressure[idx];
                    let vx = self.solver.state.vx[idx];
                    let vy = self.solver.state.vy[idx];
                    let v_mag = (vx * vx + vy * vy).sqrt();

                    if p.abs() > 0.02 || v_mag > 0.02 {
                        if p > 0.0 {
                            // Positive pressure soliton core: Golden amber / fiery white
                            let t = (p * 1.5).clamp(0.0, 1.0);
                            let r = (40.0 + t * 215.0) as u8;
                            let g = (20.0 + t * 180.0) as u8;
                            let b = (10.0 + t * 60.0) as u8;
                            self.pixel_buffer[px_idx] = r;
                            self.pixel_buffer[px_idx + 1] = g;
                            self.pixel_buffer[px_idx + 2] = b;
                        } else {
                            // Negative pressure wavefront: Electric cyan / deep blue
                            let t = (-p * 1.5).clamp(0.0, 1.0);
                            let r = (10.0 + t * 30.0) as u8;
                            let g = (20.0 + t * 140.0) as u8;
                            let b = (50.0 + t * 205.0) as u8;
                            self.pixel_buffer[px_idx] = r;
                            self.pixel_buffer[px_idx + 1] = g;
                            self.pixel_buffer[px_idx + 2] = b;
                        }
                    } else {
                        // Deep CRT dark background with faint grid ambient
                        self.pixel_buffer[px_idx] = 12;
                        self.pixel_buffer[px_idx + 1] = 15;
                        self.pixel_buffer[px_idx + 2] = 24;
                    }
                    self.pixel_buffer[px_idx + 3] = 255;
                }
            }
        }
    }
}
