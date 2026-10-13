//! Physically accurate rigid-body Pong coupled with Dzog acoustic field.

use dzog_core::*;

pub struct RigidPong {
    pub solver: AcousticSolver,
    pub width: f32,
    pub height: f32,

    // Ball kinematics
    pub ball_x: f32,
    pub ball_y: f32,
    pub ball_vx: f32,
    pub ball_vy: f32,
    pub ball_radius: f32,
    pub ball_mass: f32,
    pub restitution: f32, // Coefficient of restitution (1.0 = perfectly elastic)

    // Paddle kinematics
    pub p1_paddle_y: f32,
    pub p2_paddle_y: f32,
    pub paddle_height: f32,
    pub paddle_width: f32,
    pub p1_vy: f32,
    pub p2_vy: f32,
    pub p1_x: f32,
    pub p2_x: f32,

    // Scoring & State
    pub p1_score: u32,
    pub p2_score: u32,
    pub in_play: bool,
    pub total_impacts: usize,
}

impl RigidPong {
    pub fn new(grid_w: usize, grid_h: usize) -> Self {
        let grid = GridConfig::new(grid_w, grid_h, 1.0, 1.0);
        let state = AcousticState::new(grid);
        let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.02); // Acoustic cavity
        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        let config = SolverConfig::new(dt, 0.7, 4);

        let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();
        // Listener probes at court edges to capture impact soundscapes
        solver.add_probe(PressureProbe::new("sound_p1", 4, grid_h / 2, 0));
        solver.add_probe(PressureProbe::new("sound_p2", grid_w - 5, grid_h / 2, 0));

        let width = grid_w as f32;
        let height = grid_h as f32;
        let paddle_height = height * 0.28;
        let paddle_width = 1.8;

        Self {
            solver,
            width,
            height,
            ball_x: width / 2.0,
            ball_y: height / 2.0,
            ball_vx: 0.0,
            ball_vy: 0.0,
            ball_radius: 1.0,
            ball_mass: 1.0,
            restitution: 1.0, // Perfectly elastic default
            p1_paddle_y: height / 2.0 - paddle_height / 2.0,
            p2_paddle_y: height / 2.0 - paddle_height / 2.0,
            paddle_height,
            paddle_width,
            p1_vy: 0.0,
            p2_vy: 0.0,
            p1_x: 4.0,
            p2_x: width - 5.0,
            p1_score: 0,
            p2_score: 0,
            in_play: false,
            total_impacts: 0,
        }
    }

    /// Serve the ball with speed and angle.
    pub fn serve(&mut self, towards_player: u8) {
        self.ball_x = self.width / 2.0;
        self.ball_y = self.height / 2.0;
        let speed = 0.55f32;
        let angle = 0.25f32;

        if towards_player == 1 {
            self.ball_vx = -speed * angle.cos();
            self.ball_vy = speed * angle.sin();
        } else {
            self.ball_vx = speed * angle.cos();
            self.ball_vy = -speed * angle.sin();
        }
        self.in_play = true;
    }

    /// Move player 1 paddle.
    pub fn move_p1(&mut self, dy: f32) {
        let max_y = self.height - self.paddle_height - 1.0;
        self.p1_paddle_y = (self.p1_paddle_y + dy).clamp(1.0, max_y);
        self.p1_vy = dy;
    }

    /// Move player 2 paddle.
    pub fn move_p2(&mut self, dy: f32) {
        let max_y = self.height - self.paddle_height - 1.0;
        self.p2_paddle_y = (self.p2_paddle_y + dy).clamp(1.0, max_y);
        self.p2_vy = dy;
    }

    /// Couple ball collision impact to the acoustic field:
    /// Injects a smooth pressure impulse at the contact point.
    fn couple_acoustic_impact(&mut self, contact_x: f32, contact_y: f32, normal_impulse: f32) {
        let amplitude = normal_impulse * 0.8;
        let impulse = GaussianImpulse::new(contact_x, contact_y, 1.5, amplitude);
        impulse.inject(&mut self.solver.state);
        self.total_impacts += 1;
    }

    /// Advance game physics by time dt_frame using continuous collision detection.
    pub fn step(&mut self, dt_frame: f32) -> Option<u8> {
        if !self.in_play {
            return None;
        }

        // Substep rigid body kinematics
        let sub_steps = 8;
        let sub_dt = dt_frame / sub_steps as f32;

        let mut scorer = None;

        for _ in 0..sub_steps {
            self.ball_x += self.ball_vx * sub_dt;
            self.ball_y += self.ball_vy * sub_dt;

            // 1. Top and Bottom Wall collisions
            if self.ball_y - self.ball_radius <= 1.0 {
                self.ball_y = 1.0 + self.ball_radius;
                self.ball_vy = -self.ball_vy * self.restitution;
                let impulse = 2.0 * self.ball_mass * self.ball_vy.abs();
                self.couple_acoustic_impact(self.ball_x, 1.0, impulse);
            } else if self.ball_y + self.ball_radius >= self.height - 1.0 {
                self.ball_y = self.height - 1.0 - self.ball_radius;
                self.ball_vy = -self.ball_vy * self.restitution;
                let impulse = 2.0 * self.ball_mass * self.ball_vy.abs();
                self.couple_acoustic_impact(self.ball_x, self.height - 1.0, impulse);
            }

            // 2. Paddle 1 (Left) collision
            let p1_front_x = self.p1_x + self.paddle_width;
            if self.ball_vx < 0.0
                && self.ball_x - self.ball_radius <= p1_front_x
                && self.ball_x + self.ball_radius >= self.p1_x
                && self.ball_y >= self.p1_paddle_y - self.ball_radius
                && self.ball_y <= self.p1_paddle_y + self.paddle_height + self.ball_radius
            {
                self.ball_x = p1_front_x + self.ball_radius;
                self.ball_vx = -self.ball_vx * self.restitution;
                // Tangential friction transfer from moving paddle
                self.ball_vy += self.p1_vy * 0.35;
                let impulse = (1.0 + self.restitution) * self.ball_mass * self.ball_vx.abs();
                self.couple_acoustic_impact(p1_front_x, self.ball_y, impulse);
            }

            // 3. Paddle 2 (Right) collision
            let p2_front_x = self.p2_x;
            if self.ball_vx > 0.0
                && self.ball_x + self.ball_radius >= p2_front_x
                && self.ball_x - self.ball_radius <= p2_front_x + self.paddle_width
                && self.ball_y >= self.p2_paddle_y - self.ball_radius
                && self.ball_y <= self.p2_paddle_y + self.paddle_height + self.ball_radius
            {
                self.ball_x = p2_front_x - self.ball_radius;
                self.ball_vx = -self.ball_vx * self.restitution;
                self.ball_vy += self.p2_vy * 0.35;
                let impulse = (1.0 + self.restitution) * self.ball_mass * self.ball_vx.abs();
                self.couple_acoustic_impact(p2_front_x, self.ball_y, impulse);
            }

            // 4. Goal scoring checks
            if self.ball_x < 1.0 {
                // Ball passed player 1 -> Player 2 scores
                self.p2_score += 1;
                self.in_play = false;
                scorer = Some(2);
                break;
            } else if self.ball_x > self.width - 1.0 {
                // Ball passed player 2 -> Player 1 scores
                self.p1_score += 1;
                self.in_play = false;
                scorer = Some(1);
                break;
            }
        }

        // Advance Dzog acoustic field substeps
        self.solver.advance_macro();

        scorer
    }

    /// Calculate total mechanical energy of the ball: E_kin = 0.5 * m * (vx^2 + vy^2).
    pub fn kinetic_energy(&self) -> f32 {
        0.5 * self.ball_mass * (self.ball_vx * self.ball_vx + self.ball_vy * self.ball_vy)
    }
}
