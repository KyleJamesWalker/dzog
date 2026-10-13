//! Field-Wave Pong: Ball as an acoustic wave packet on a 2D Dzog staggered lattice.

use dzog_core::*;

pub struct FieldWavePong {
    pub solver: AcousticSolver,
    pub p1_paddle_y: usize,
    pub p2_paddle_y: usize,
    pub paddle_height: usize,
    pub p1_vy: f32,
    pub p2_vy: f32,
    pub p1_score: u32,
    pub p2_score: u32,
    pub goal_probe_left_idx: usize,
    pub goal_probe_right_idx: usize,
    pub in_play: bool,
}

impl FieldWavePong {
    pub fn new(width: usize, height: usize) -> Self {
        let grid = GridConfig::new(width, height, 1.0, 1.0);
        let state = AcousticState::new(grid);
        // Lossless cavity with slight sponge damping in goal nets
        let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.001);

        // Apply light damping behind goal lines to prevent infinite reflections
        material.set_region(0, 4, 0, height, 1.0, 1.0, 0.05);
        material.set_region(width - 4, width, 0, height, 1.0, 1.0, 0.05);

        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        let config = SolverConfig::new(dt, 0.7, 4);

        let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

        // Place acoustic goal probes in the back corners behind each paddle
        let probe_left = PressureProbe::new("goal_p1_detector", 2, height / 2, 1).with_threshold(0.12);
        let probe_right = PressureProbe::new("goal_p2_detector", width - 3, height / 2, 1).with_threshold(0.12);

        solver.add_probe(probe_left);
        solver.add_probe(probe_right);

        let paddle_height = (height / 4).max(4);
        let mut pong = Self {
            solver,
            p1_paddle_y: height / 2 - paddle_height / 2,
            p2_paddle_y: height / 2 - paddle_height / 2,
            paddle_height,
            p1_vy: 0.0,
            p2_vy: 0.0,
            p1_score: 0,
            p2_score: 0,
            goal_probe_left_idx: 0,
            goal_probe_right_idx: 1,
            in_play: false,
        };

        pong.update_paddles_in_field();
        pong
    }

    /// Synchronize paddle moving boundaries with the Dzog field solver.
    pub fn update_paddles_in_field(&mut self) {
        let w = self.solver.state.config.width;
        let h = self.solver.state.config.height;

        // Clear existing interior walls
        self.solver.boundary.clear_interior();

        // Paddle 1: placed at column x = 5
        let p1_x = 5;
        let p1_y_end = (self.p1_paddle_y + self.paddle_height).min(h - 1);
        for y in self.p1_paddle_y..=p1_y_end {
            let idx = self.solver.state.config.index(p1_x, y);
            self.solver.boundary.wall_east[idx] = true;
            // Enforce moving boundary normal/tangential velocities
            self.solver.boundary.wall_vx[idx] = 0.0;
            self.solver.boundary.wall_vy[idx] = self.p1_vy;
        }

        // Paddle 2: placed at column x = w - 7
        let p2_x = w - 7;
        let p2_y_end = (self.p2_paddle_y + self.paddle_height).min(h - 1);
        for y in self.p2_paddle_y..=p2_y_end {
            let idx = self.solver.state.config.index(p2_x, y);
            self.solver.boundary.wall_east[idx] = true;
            self.solver.boundary.wall_vx[idx] = 0.0;
            self.solver.boundary.wall_vy[idx] = self.p2_vy;
        }
    }

    /// Move player 1 paddle (left).
    pub fn move_p1(&mut self, dy: isize) {
        let h = self.solver.state.config.height;
        let max_y = h.saturating_sub(self.paddle_height + 1);
        let new_y = (self.p1_paddle_y as isize + dy).clamp(1, max_y as isize) as usize;
        self.p1_vy = (dy as f32) * 0.4;
        self.p1_paddle_y = new_y;
        self.update_paddles_in_field();
    }

    /// Move player 2 paddle (right).
    pub fn move_p2(&mut self, dy: isize) {
        let h = self.solver.state.config.height;
        let max_y = h.saturating_sub(self.paddle_height + 1);
        let new_y = (self.p2_paddle_y as isize + dy).clamp(1, max_y as isize) as usize;
        self.p2_vy = (dy as f32) * 0.4;
        self.p2_paddle_y = new_y;
        self.update_paddles_in_field();
    }

    /// Serve a new acoustic wave packet from court center towards direction (angle_rad).
    pub fn serve(&mut self, towards_player: u8) {
        let w = self.solver.state.config.width;
        let h = self.solver.state.config.height;

        // Clear field state
        self.solver.state.clear();
        self.solver.probes[self.goal_probe_left_idx].clear();
        self.solver.probes[self.goal_probe_right_idx].clear();

        let cx = w as f32 / 2.0;
        let cy = h as f32 / 2.0;
        let angle = if towards_player == 1 {
            std::f32::consts::PI + 0.15 // Towards left
        } else {
            -0.15 // Towards right
        };

        let mut packet = DirectionalWavePacket::new(cx, cy, 2.2, 1.2, angle);
        packet.inject(&mut self.solver.state, &self.solver.material);
        self.in_play = true;
    }

    /// Step simulation forward by one macro tick and check for goal events.
    pub fn step(&mut self) -> Option<u8> {
        if !self.in_play {
            return None;
        }

        self.solver.advance_macro();

        // Check acoustic goal probes
        let left_triggered = self.solver.probes[self.goal_probe_left_idx].is_triggered();
        let right_triggered = self.solver.probes[self.goal_probe_right_idx].is_triggered();

        if left_triggered {
            // Ball passed player 1's paddle into left net -> Player 2 scores!
            self.p2_score += 1;
            self.in_play = false;
            Some(2)
        } else if right_triggered {
            // Ball passed player 2's paddle into right net -> Player 1 scores!
            self.p1_score += 1;
            self.in_play = false;
            Some(1)
        } else {
            None
        }
    }

    /// Get current peak pressure location (to track wave packet center).
    pub fn wave_packet_centroid(&self) -> (f32, f32) {
        let w = self.solver.state.config.width;
        let h = self.solver.state.config.height;
        let mut max_p = 0.0f32;
        let mut best_x = w as f32 / 2.0;
        let mut best_y = h as f32 / 2.0;

        for y in 0..h {
            for x in 0..w {
                let idx = self.solver.state.config.index(x, y);
                let p = self.solver.state.pressure[idx].abs();
                if p > max_p {
                    max_p = p;
                    best_x = x as f32;
                    best_y = y as f32;
                }
            }
        }
        (best_x, best_y)
    }
}
