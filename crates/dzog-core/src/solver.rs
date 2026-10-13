//! Yee-staggered finite-difference time-domain (FDTD) reference acoustic solver.

use serde::{Deserialize, Serialize};
use crate::boundary::{BoundaryMask, OuterBoundaryRule};
use crate::grid::{AcousticState, GridConfig};
use crate::material::MaterialField;
use crate::probe::PressureProbe;

/// Nonlinear constitutive field parameters for solitary wave cohesion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NonlinearLaw {
    /// Soliton focusing strength (0.0 = pure linear acoustics).
    pub beta: f32,
    /// Saturation parameter preventing blowup.
    pub alpha: f32,
    /// Sub-threshold background decay rate.
    pub threshold_decay: f32,
}

impl Default for NonlinearLaw {
    fn default() -> Self {
        Self {
            beta: 0.0,
            alpha: 1.0,
            threshold_decay: 0.0,
        }
    }
}

/// Solver configuration and time-step discretization parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Time step size dt (seconds or normalized time units).
    pub dt: f32,
    /// CFL safety factor (0 < safety < 1, typically 0.7).
    pub cfl_safety: f32,
    /// Number of substeps advanced per frame or macro-step.
    pub substeps_per_macro: usize,
    /// Nonlinear solitary field law parameters.
    pub nonlinearity: NonlinearLaw,
}

impl SolverConfig {
    /// Calculate maximum stable CFL time step:
    /// dt_cfl = safety / (c_bound * sqrt(1/dx^2 + 1/dy^2))
    pub fn compute_cfl_dt(grid: &GridConfig, material: &MaterialField, safety: f32) -> f32 {
        assert!(safety > 0.0 && safety < 1.0, "CFL safety must be between 0 and 1");
        let c_bound = material.max_wave_speed_bound();
        let inv_dx2 = 1.0 / (grid.dx * grid.dx);
        let inv_dy2 = 1.0 / (grid.dy * grid.dy);
        let spatial_factor = (inv_dx2 + inv_dy2).sqrt();
        safety / (c_bound * spatial_factor)
    }

    pub fn new(dt: f32, cfl_safety: f32, substeps_per_macro: usize) -> Self {
        assert!(dt > 0.0 && dt.is_finite(), "dt must be positive and finite");
        assert!(cfl_safety > 0.0 && cfl_safety < 1.0, "cfl_safety must be in (0, 1)");
        assert!(substeps_per_macro >= 1, "substeps_per_macro must be >= 1");
        Self {
            dt,
            cfl_safety,
            substeps_per_macro,
            nonlinearity: NonlinearLaw::default(),
        }
    }

    pub fn with_nonlinearity(mut self, nonlinearity: NonlinearLaw) -> Self {
        self.nonlinearity = nonlinearity;
        self
    }
}

/// Unconditionally positive Padé/Taylor exponential decay exp(-x) for x >= 0.
/// Matches IEEE-754 expf(-x) within 2e-6 for typical FDTD step losses (gamma * dt < 0.05)
/// while avoiding scalar libm call overhead in tight lattice loops.
#[inline(always)]
fn fast_neg_exp(x: f32) -> f32 {
    if x <= 0.0 {
        1.0
    } else if x < 0.08 {
        1.0 / (1.0 + x * (1.0 + 0.5 * x * (1.0 + 0.33333334 * x)))
    } else {
        (-x).exp()
    }
}

/// The core acoustic field engine simulation runner.
pub struct AcousticSolver {
    pub state: AcousticState,
    pub material: MaterialField,
    pub boundary: BoundaryMask,
    pub config: SolverConfig,
    pub probes: Vec<PressureProbe>,

    /// Current simulation integer step index.
    pub current_step: usize,
    /// Current accumulated simulation time.
    pub current_time: f32,
}

impl AcousticSolver {
    pub fn new(
        state: AcousticState,
        material: MaterialField,
        boundary: BoundaryMask,
        config: SolverConfig,
    ) -> Result<Self, String> {
        state.validate()?;
        material.validate()?;

        // Verify CFL stability condition
        let max_dt = SolverConfig::compute_cfl_dt(&state.config, &material, config.cfl_safety);
        if config.dt > max_dt {
            return Err(format!(
                "Configured dt={} exceeds stable CFL bound dt_cfl={:.6} (safety={})",
                config.dt, max_dt, config.cfl_safety
            ));
        }

        Ok(Self {
            state,
            material,
            boundary,
            config,
            probes: Vec::new(),
            current_step: 0,
            current_time: 0.0,
        })
    }

    pub fn add_probe(&mut self, probe: PressureProbe) {
        self.probes.push(probe);
    }

    /// Advance a single numerical substep.
    ///
    /// 1. Staggered velocity update: v(n-1/2) -> v(n+1/2) using -grad(p^n) and exponential damping.
    /// 2. Apply wall and moving boundary conditions to faces.
    /// 3. Staggered pressure update: p(n) -> p(n+1) using -K div(v^{n+1/2}) and exponential damping.
    /// 4. Sample pressure probes.
    pub fn step(&mut self) {
        let dt = self.config.dt;
        let w = self.state.config.width;
        let h = self.state.config.height;
        let dx = self.state.config.dx;
        let dy = self.state.config.dy;
        let inv_dx = 1.0 / dx;
        let inv_dy = 1.0 / dy;

        // -------------------------------------------------------------
        // Pass 1: Staggered velocity half-step update
        // -------------------------------------------------------------
        // Horizontal velocity vx on East faces
        for y in 0..h {
            for x in 0..w {
                let idx = self.state.config.index(x, y);

                if self.boundary.wall_east[idx] {
                    // Impermeable / moving wall condition
                    self.state.vx[idx] = self.boundary.wall_vx[idx];
                } else if x + 1 < w {
                    // Interior face between (x, y) and (x + 1, y)
                    let idx_r = self.state.config.index(x + 1, y);
                    let grad_p = (self.state.pressure[idx_r] - self.state.pressure[idx]) * inv_dx;
                    let rho_face = self.material.face_density_east(x, y);
                    let gamma_face = self.material.face_damping_east(x, y);

                    // Dissipative exponential factor
                    let decay = fast_neg_exp(gamma_face * dt);
                    let accel = -grad_p / rho_face;
                    self.state.vx[idx] = decay * self.state.vx[idx] + accel * dt;
                } else {
                    // Outer east boundary face
                    match self.boundary.outer_rule {
                        OuterBoundaryRule::RigidWall | OuterBoundaryRule::AbsorbingSponge => {
                            self.state.vx[idx] = 0.0;
                        }
                        OuterBoundaryRule::Periodic => {
                            let idx_wrap = self.state.config.index(0, y);
                            let grad_p = (self.state.pressure[idx_wrap] - self.state.pressure[idx]) * inv_dx;
                            let rho_face = self.material.density[idx];
                            let gamma_face = self.material.damping[idx];
                            let decay = fast_neg_exp(gamma_face * dt);
                            let accel = -grad_p / rho_face;
                            self.state.vx[idx] = decay * self.state.vx[idx] + accel * dt;
                        }
                    }
                }
            }
        }

        // Vertical velocity vy on North faces
        for y in 0..h {
            for x in 0..w {
                let idx = self.state.config.index(x, y);

                if self.boundary.wall_north[idx] {
                    // Impermeable / moving wall condition
                    self.state.vy[idx] = self.boundary.wall_vy[idx];
                } else if y + 1 < h {
                    // Interior face between (x, y) and (x, y + 1)
                    let idx_t = self.state.config.index(x, y + 1);
                    let grad_p = (self.state.pressure[idx_t] - self.state.pressure[idx]) * inv_dy;
                    let rho_face = self.material.face_density_north(x, y);
                    let gamma_face = self.material.face_damping_north(x, y);

                    let decay = fast_neg_exp(gamma_face * dt);
                    let accel = -grad_p / rho_face;
                    self.state.vy[idx] = decay * self.state.vy[idx] + accel * dt;
                } else {
                    // Outer north boundary face
                    match self.boundary.outer_rule {
                        OuterBoundaryRule::RigidWall | OuterBoundaryRule::AbsorbingSponge => {
                            self.state.vy[idx] = 0.0;
                        }
                        OuterBoundaryRule::Periodic => {
                            let idx_wrap = self.state.config.index(x, 0);
                            let grad_p = (self.state.pressure[idx_wrap] - self.state.pressure[idx]) * inv_dy;
                            let rho_face = self.material.density[idx];
                            let gamma_face = self.material.damping[idx];
                            let decay = fast_neg_exp(gamma_face * dt);
                            let accel = -grad_p / rho_face;
                            self.state.vy[idx] = decay * self.state.vy[idx] + accel * dt;
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // Pass 2: Staggered pressure integer-step update
        // -------------------------------------------------------------
        let has_cohesion = self.material.cohesion.len() == w * h
            && self.material.yield_pressure.len() == w * h;

        for y in 0..h {
            for x in 0..w {
                let idx = self.state.config.index(x, y);

                // East face velocity
                let vx_e = self.state.vx[idx];
                // West face velocity
                let vx_w = if x > 0 {
                    let idx_l = self.state.config.index(x - 1, y);
                    self.state.vx[idx_l]
                } else {
                    match self.boundary.outer_rule {
                        OuterBoundaryRule::RigidWall | OuterBoundaryRule::AbsorbingSponge => 0.0,
                        OuterBoundaryRule::Periodic => {
                            let idx_wrap = self.state.config.index(w - 1, y);
                            self.state.vx[idx_wrap]
                        }
                    }
                };

                // North face velocity
                let vy_n = self.state.vy[idx];
                // South face velocity
                let vy_s = if y > 0 {
                    let idx_b = self.state.config.index(x, y - 1);
                    self.state.vy[idx_b]
                } else {
                    match self.boundary.outer_rule {
                        OuterBoundaryRule::RigidWall | OuterBoundaryRule::AbsorbingSponge => 0.0,
                        OuterBoundaryRule::Periodic => {
                            let idx_wrap = self.state.config.index(x, h - 1);
                            self.state.vy[idx_wrap]
                        }
                    }
                };

                // Divergence of velocity
                let div_v = (vx_e - vx_w) * inv_dx + (vy_n - vy_s) * inv_dy;

                let k = self.material.bulk_modulus[idx];
                let gamma = self.material.damping[idx];
                let decay = fast_neg_exp(gamma * dt);

                let p_linear = decay * self.state.pressure[idx] - (k * dt) * div_v;

                // Nonlinear solitary field dynamics in open medium;
                // never amplify static pressure inside solid-state structures or blocked boundary walls.
                let is_solid_or_wall = (has_cohesion && self.material.cohesion[idx] > 0.0)
                    || self.boundary.wall_east[idx]
                    || self.boundary.wall_north[idx];
                let p_new = if self.config.nonlinearity.beta > 0.0 && !is_solid_or_wall {
                    let beta = self.config.nonlinearity.beta;
                    let alpha = self.config.nonlinearity.alpha;
                    let kappa = self.config.nonlinearity.threshold_decay;
                    let nl_flux = (beta * p_linear) / (1.0 + alpha * p_linear * p_linear) - kappa * p_linear;
                    p_linear + nl_flux * dt
                } else {
                    p_linear
                };

                self.state.pressure[idx] = p_new;

                // Continuous cohesion erosion under acoustic overpressure
                if has_cohesion {
                    let c = self.material.cohesion[idx];
                    let p_yield = self.material.yield_pressure[idx];
                    if c > 0.0 && p_new.abs() > p_yield {
                        let excess = p_new.abs() - p_yield;
                        let new_c = (c - 0.35 * excess * dt).max(0.0);
                        self.material.cohesion[idx] = new_c;
                    }
                }
            }
        }

        // Advance simulation clock
        self.current_step += 1;
        self.current_time += dt;

        // Sample listener probes
        for probe in &mut self.probes {
            probe.sample(&self.state, self.current_step, self.current_time);
        }
    }

    /// Advance simulation by `substeps_per_macro` substeps.
    pub fn advance_macro(&mut self) {
        for _ in 0..self.config.substeps_per_macro {
            self.step();
        }
    }
}
