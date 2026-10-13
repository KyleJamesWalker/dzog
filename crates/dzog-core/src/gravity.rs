//! Long-range scalar gravitational potential layer Phi_g(x, y) on the Dzog lattice.
//!
//! Computes gravitational potential wells from excess material mass density (rho > rho_ambient)
//! and singularity attractors via iterative Jacobi/Gauss-Seidel Poisson relaxation:
//!     Laplacian(Phi_g) = G * (rho(x, y) - rho_ambient)
//! Actors and wave-guided entities experience gravitational acceleration g = -grad(Phi_g).

use serde::{Deserialize, Serialize};
use crate::grid::GridConfig;
use crate::material::MaterialField;

/// A localized gravitational singularity or planetary attractor well.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GravityWell {
    pub x: f32,
    pub y: f32,
    /// Gravitational strength (positive = attractive well, negative = repulsor).
    pub strength: f32,
    /// Softening core radius to prevent infinite force at r -> 0.
    pub core_radius: f32,
}

/// Scalar gravitational potential field Phi_g(x, y) over the lattice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GravityField {
    pub config: GridConfig,
    /// Gravitational potential Phi_g(x, y) at cell centers (negative inside attractive wells).
    pub potential: Vec<f32>,
    /// Scratch buffer for Poisson relaxation.
    #[serde(skip)]
    scratch: Vec<f32>,
    /// Coupling constant G for self-gravitating material density contrasts.
    pub coupling_g: f32,
    /// Ambient vacuum density baseline (default 1.0).
    pub ambient_rho: f32,
    /// Explicit singularity wells (e.g. central star / black hole in Star Control mode).
    pub wells: Vec<GravityWell>,
}

impl GravityField {
    pub fn new(config: GridConfig, coupling_g: f32) -> Self {
        let count = config.cell_count();
        Self {
            config,
            potential: vec![0.0; count],
            scratch: vec![0.0; count],
            coupling_g,
            ambient_rho: 1.0,
            wells: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.potential.fill(0.0);
    }

    /// Perform `iterations` steps of Red-Black / Jacobi Poisson relaxation:
    /// Laplacian(Phi_g) = coupling_g * (rho - ambient_rho) + well sources.
    pub fn relax_poisson(&mut self, material: &MaterialField, iterations: usize, periodic: bool) {
        let w = self.config.width;
        let h = self.config.height;
        let count = w * h;
        if self.scratch.len() != count {
            self.scratch.resize(count, 0.0);
        }

        let dx2 = self.config.dx * self.config.dx;

        for _ in 0..iterations {
            for y in 0..h {
                for x in 0..w {
                    let idx = self.config.index(x, y);
                    if !periodic && (x == 0 || x + 1 == w || y == 0 || y + 1 == h) {
                        self.scratch[idx] = 0.0;
                        continue;
                    }

                    let xe = (x + 1) % w;
                    let xw = (x + w - 1) % w;
                    let yn = (y + 1) % h;
                    let ys = (y + h - 1) % h;

                    let phi_e = self.potential[self.config.index(xe, y)];
                    let phi_w = self.potential[self.config.index(xw, y)];
                    let phi_n = self.potential[self.config.index(x, yn)];
                    let phi_s = self.potential[self.config.index(x, ys)];

                    let excess_rho = (material.density[idx] - self.ambient_rho).max(0.0);
                    let mut source = self.coupling_g * excess_rho;

                    for well in &self.wells {
                        let mut dx = x as f32 - well.x;
                        let mut dy = y as f32 - well.y;
                        if periodic {
                            let wf = w as f32;
                            let hf = h as f32;
                            if dx > 0.5 * wf { dx -= wf; } else if dx < -0.5 * wf { dx += wf; }
                            if dy > 0.5 * hf { dy -= hf; } else if dy < -0.5 * hf { dy += hf; }
                        }
                        let r2 = dx * dx + dy * dy;
                        let r0_sq = well.core_radius * well.core_radius;
                        if r2 <= 4.0 * r0_sq {
                            source += well.strength * (-r2 / r0_sq.max(0.5)).exp();
                        }
                    }

                    // Discrete 2D Poisson update with slight screening decay (0.992) to keep periodic zero-mode bounded
                    self.scratch[idx] = 0.992 * 0.25 * (phi_e + phi_w + phi_n + phi_s - dx2 * source);
                }
            }
            std::mem::swap(&mut self.potential, &mut self.scratch);
        }
    }

    /// Evaluate gravitational acceleration vector g = -grad(Phi_g) + analytical well pull at `(xf, yf)`.
    pub fn acceleration_at(&self, xf: f32, yf: f32, periodic: bool) -> (f32, f32) {
        let w = self.config.width;
        let h = self.config.height;
        let x = (xf.round() as isize).rem_euclid(w as isize) as usize;
        let y = (yf.round() as isize).rem_euclid(h as isize) as usize;

        let mut gx = 0.0f32;
        let mut gy = 0.0f32;

        if periodic || (x > 0 && x + 1 < w && y > 0 && y + 1 < h) {
            let xe = (x + 1) % w;
            let xw = (x + w - 1) % w;
            let yn = (y + 1) % h;
            let ys = (y + h - 1) % h;

            let phi_e = self.potential[self.config.index(xe, y)];
            let phi_w = self.potential[self.config.index(xw, y)];
            let phi_n = self.potential[self.config.index(x, yn)];
            let phi_s = self.potential[self.config.index(x, ys)];

            gx -= (phi_e - phi_w) / (2.0 * self.config.dx);
            gy -= (phi_n - phi_s) / (2.0 * self.config.dy);
        }

        // Smooth analytical softened inverse-distance pull from explicit singularity wells
        for well in &self.wells {
            let mut dx = well.x - xf;
            let mut dy = well.y - yf;
            if periodic {
                let wf = w as f32;
                let hf = h as f32;
                if dx > 0.5 * wf { dx -= wf; } else if dx < -0.5 * wf { dx += wf; }
                if dy > 0.5 * hf { dy -= hf; } else if dy < -0.5 * hf { dy += hf; }
            }
            let r2 = dx * dx + dy * dy + well.core_radius * well.core_radius;
            let r = r2.sqrt();
            let accel_mag = well.strength / r2;
            gx += (dx / r) * accel_mag;
            gy += (dy / r) * accel_mag;
        }

        (gx, gy)
    }
}
