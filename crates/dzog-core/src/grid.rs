//! Spatially staggered 2D finite-difference grid layout.
//!
//! Following Dzog's acoustic discretization:
//! - Center: cell center (i + 0.5, j + 0.5) stores scalar pressure p, density rho, bulk modulus K, damping gamma.
//! - East face: vertical face between (i, j) and (i + 1, j) stores horizontal velocity vx.
//! - North face: horizontal face between (i, j) and (i, j + 1) stores vertical velocity vy.

use serde::{Deserialize, Serialize};

/// Grid dimensions and spatial discretization spacing.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridConfig {
    /// Number of horizontal cells.
    pub width: usize,
    /// Number of vertical cells.
    pub height: usize,
    /// Spatial grid spacing in x (meters or normalized units).
    pub dx: f32,
    /// Spatial grid spacing in y (meters or normalized units).
    pub dy: f32,
}

impl GridConfig {
    pub fn new(width: usize, height: usize, dx: f32, dy: f32) -> Self {
        assert!(width >= 2, "Width must be at least 2 cells");
        assert!(height >= 2, "Height must be at least 2 cells");
        assert!(dx > 0.0 && dx.is_finite(), "dx must be positive and finite");
        assert!(dy > 0.0 && dy.is_finite(), "dy must be positive and finite");
        Self { width, height, dx, dy }
    }

    /// Total number of cells.
    #[inline]
    pub fn cell_count(&self) -> usize {
        self.width * self.height
    }

    /// Linear index of cell (x, y).
    #[inline]
    pub fn index(&self, x: usize, y: usize) -> usize {
        debug_assert!(x < self.width && y < self.height, "Coords out of bounds: ({x}, {y})");
        y * self.width + x
    }

    /// Coordinate pair from linear index.
    #[inline]
    pub fn coords(&self, idx: usize) -> (usize, usize) {
        debug_assert!(idx < self.cell_count());
        (idx % self.width, idx / self.width)
    }

    /// Check if coordinates are within the grid bounds.
    #[inline]
    pub fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.width && y < self.height
    }
}

/// Dynamic acoustic state fields allocated on the staggered grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcousticState {
    pub config: GridConfig,
    /// Center scalar pressure perturbation p(x, y). Size: width * height.
    pub pressure: Vec<f32>,
    /// East-face horizontal acoustic particle velocity vx(x, y). Size: width * height.
    /// Evaluated on the vertical face between cell (x, y) and (x + 1, y).
    pub vx: Vec<f32>,
    /// North-face vertical acoustic particle velocity vy(x, y). Size: width * height.
    /// Evaluated on the horizontal face between cell (x, y) and (x, y + 1).
    pub vy: Vec<f32>,
    /// Center exciton / radiative luminescence field L(x, y) >= 0 driven by wave energy,
    /// Bohmian curvature |Q|, and piezoelectric crystal stress. Size: width * height.
    #[serde(default)]
    pub luminescence: Vec<f32>,
}

impl AcousticState {
    /// Initialize a zero-state field.
    pub fn new(config: GridConfig) -> Self {
        let count = config.cell_count();
        Self {
            config,
            pressure: vec![0.0; count],
            vx: vec![0.0; count],
            vy: vec![0.0; count],
            luminescence: vec![0.0; count],
        }
    }

    /// Clears all fields to zero.
    pub fn clear(&mut self) {
        self.pressure.fill(0.0);
        self.vx.fill(0.0);
        self.vy.fill(0.0);
        self.luminescence.fill(0.0);
    }

    /// Validate that all values are finite.
    pub fn validate(&self) -> Result<(), String> {
        let count = self.config.cell_count();
        if self.pressure.len() != count || self.vx.len() != count || self.vy.len() != count {
            return Err("Buffer lengths do not match grid dimensions".to_string());
        }
        for (i, (&p, (&vx, &vy))) in self.pressure.iter().zip(self.vx.iter().zip(self.vy.iter())).enumerate() {
            if !p.is_finite() || !vx.is_finite() || !vy.is_finite() {
                let (x, y) = self.config.coords(i);
                return Err(format!("Non-finite field state at ({x}, {y}): p={p}, vx={vx}, vy={vy}"));
            }
        }
        Ok(())
    }

    /// Central-difference scalar pressure gradient grad(p) = (dp/dx, dp/dy) at interior cell (x, y).
    #[inline]
    pub fn pressure_gradient_at(&self, x: usize, y: usize) -> (f32, f32) {
        let w = self.config.width;
        let h = self.config.height;
        if x == 0 || x + 1 >= w || y == 0 || y + 1 >= h {
            return (0.0, 0.0);
        }
        let p_e = self.pressure[self.config.index(x + 1, y)];
        let p_w = self.pressure[self.config.index(x - 1, y)];
        let p_n = self.pressure[self.config.index(x, y + 1)];
        let p_s = self.pressure[self.config.index(x, y - 1)];

        let gx = (p_e - p_w) / (2.0 * self.config.dx);
        let gy = (p_n - p_s) / (2.0 * self.config.dy);
        (gx, gy)
    }

    /// Regularized field amplitude envelope R(x, y) = sqrt(p^2 + eps^2).
    #[inline]
    pub fn envelope_at(&self, x: usize, y: usize, eps: f32) -> f32 {
        let p = self.pressure[self.config.index(x, y)];
        (p * p + eps * eps).sqrt()
    }

    /// Bohm-Hiley Quantum Potential Q(x, y) = - (Laplacian R) / R at interior cell (x, y).
    ///
    /// Operationalizes Bohm-Hiley active information (`philosophy.md`):
    /// Q depends on the spatial curvature (form) of the wave envelope rather than raw amplitude.
    /// Regions of constructive wave focusing produce negative Q wells (attracting wave-guided trajectories).
    #[inline]
    pub fn quantum_potential_at(&self, x: usize, y: usize) -> f32 {
        let w = self.config.width;
        let h = self.config.height;
        if x == 0 || x + 1 >= w || y == 0 || y + 1 >= h {
            return 0.0;
        }
        let eps = 0.08f32;
        let r_c = self.envelope_at(x, y, eps);
        let r_e = self.envelope_at(x + 1, y, eps);
        let r_w = self.envelope_at(x - 1, y, eps);
        let r_n = self.envelope_at(x, y + 1, eps);
        let r_s = self.envelope_at(x, y - 1, eps);

        let inv_dx2 = 1.0 / (self.config.dx * self.config.dx);
        let inv_dy2 = 1.0 / (self.config.dy * self.config.dy);
        let lap_r = (r_e - 2.0 * r_c + r_w) * inv_dx2 + (r_n - 2.0 * r_c + r_s) * inv_dy2;
        -lap_r / r_c
    }

    /// Spatial gradient of the Bohmian Quantum Potential grad(Q) = (dQ/dx, dQ/dy).
    /// The associated Bohmian guidance force is F_Q = -grad(Q).
    #[inline]
    pub fn quantum_force_at(&self, x: usize, y: usize) -> (f32, f32) {
        let w = self.config.width;
        let h = self.config.height;
        if x < 2 || x + 2 >= w || y < 2 || y + 2 >= h {
            return (0.0, 0.0);
        }
        let q_e = self.quantum_potential_at(x + 1, y);
        let q_w = self.quantum_potential_at(x - 1, y);
        let q_n = self.quantum_potential_at(x, y + 1);
        let q_s = self.quantum_potential_at(x, y - 1);

        let dqx = (q_e - q_w) / (2.0 * self.config.dx);
        let dqy = (q_n - q_s) / (2.0 * self.config.dy);
        (-dqx, -dqy)
    }

    /// Central-difference scalar pressure gradient with optional periodic wrapping.
    #[inline]
    pub fn pressure_gradient_wrapped_at(&self, x: usize, y: usize, periodic: bool) -> (f32, f32) {
        if !periodic {
            return self.pressure_gradient_at(x, y);
        }
        let w = self.config.width;
        let h = self.config.height;
        let xe = (x + 1) % w;
        let xw = (x + w - 1) % w;
        let yn = (y + 1) % h;
        let ys = (y + h - 1) % h;

        let p_e = self.pressure[self.config.index(xe, y)];
        let p_w = self.pressure[self.config.index(xw, y)];
        let p_n = self.pressure[self.config.index(x, yn)];
        let p_s = self.pressure[self.config.index(x, ys)];

        let gx = (p_e - p_w) / (2.0 * self.config.dx);
        let gy = (p_n - p_s) / (2.0 * self.config.dy);
        (gx, gy)
    }

    /// Bohm-Hiley Quantum Potential Q(x, y) = -(Laplacian R)/R with optional periodic wrapping.
    #[inline]
    pub fn quantum_potential_wrapped_at(&self, x: usize, y: usize, periodic: bool) -> f32 {
        if !periodic {
            return self.quantum_potential_at(x, y);
        }
        let w = self.config.width;
        let h = self.config.height;
        let xe = (x + 1) % w;
        let xw = (x + w - 1) % w;
        let yn = (y + 1) % h;
        let ys = (y + h - 1) % h;

        let eps = 0.08f32;
        let r_c = self.envelope_at(x, y, eps);
        let r_e = self.envelope_at(xe, y, eps);
        let r_w = self.envelope_at(xw, y, eps);
        let r_n = self.envelope_at(x, yn, eps);
        let r_s = self.envelope_at(x, ys, eps);

        let inv_dx2 = 1.0 / (self.config.dx * self.config.dx);
        let inv_dy2 = 1.0 / (self.config.dy * self.config.dy);
        let lap_r = (r_e - 2.0 * r_c + r_w) * inv_dx2 + (r_n - 2.0 * r_c + r_s) * inv_dy2;
        -lap_r / r_c
    }

    /// Bohmian guidance force F_Q = -grad(Q) with optional periodic wrapping.
    #[inline]
    pub fn quantum_force_wrapped_at(&self, x: usize, y: usize, periodic: bool) -> (f32, f32) {
        if !periodic {
            return self.quantum_force_at(x, y);
        }
        let w = self.config.width;
        let h = self.config.height;
        let xe = (x + 1) % w;
        let xw = (x + w - 1) % w;
        let yn = (y + 1) % h;
        let ys = (y + h - 1) % h;

        let q_e = self.quantum_potential_wrapped_at(xe, y, true);
        let q_w = self.quantum_potential_wrapped_at(xw, y, true);
        let q_n = self.quantum_potential_wrapped_at(x, yn, true);
        let q_s = self.quantum_potential_wrapped_at(x, ys, true);

        let dqx = (q_e - q_w) / (2.0 * self.config.dx);
        let dqy = (q_n - q_s) / (2.0 * self.config.dy);
        (-dqx, -dqy)
    }

    /// Instantaneous acoustic energy flux (Poynting vector) I = p * v at cell (x, y).
    #[inline]
    pub fn energy_flux_at(&self, x: usize, y: usize) -> (f32, f32) {
        let idx = self.config.index(x, y);
        let p = self.pressure[idx];
        (p * self.vx[idx], p * self.vy[idx])
    }

    /// 2D scalar vorticity (curl of particle velocity: dv_y/dx - dv_x/dy) at interior cell (x, y).
    #[inline]
    pub fn vorticity_at(&self, x: usize, y: usize) -> f32 {
        let w = self.config.width;
        let h = self.config.height;
        if x + 1 >= w || y + 1 >= h {
            return 0.0;
        }
        let idx = self.config.index(x, y);
        let idx_r = self.config.index(x + 1, y);
        let idx_t = self.config.index(x, y + 1);

        let dvy_dx = (self.vy[idx_r] - self.vy[idx]) / self.config.dx;
        let dvx_dy = (self.vx[idx_t] - self.vx[idx]) / self.config.dy;
        dvy_dx - dvx_dy
    }

    /// Central-difference scalar luminescence gradient grad(L) = (dL/dx, dL/dy) with optional periodic wrapping.
    /// Used by cohesive crystal bodies to evaluate dynamic radiative illumination from nearby solitons and blasts.
    #[inline]
    pub fn luminescence_gradient_wrapped_at(&self, x: usize, y: usize, periodic: bool) -> (f32, f32) {
        let w = self.config.width;
        let h = self.config.height;
        if self.luminescence.len() != w * h {
            return (0.0, 0.0);
        }
        if !periodic && (x == 0 || x + 1 >= w || y == 0 || y + 1 >= h) {
            return (0.0, 0.0);
        }
        let xe = (x + 1) % w;
        let xw = (x + w - 1) % w;
        let yn = (y + 1) % h;
        let ys = (y + h - 1) % h;

        let l_e = self.luminescence[self.config.index(xe, y)];
        let l_w = self.luminescence[self.config.index(xw, y)];
        let l_n = self.luminescence[self.config.index(x, yn)];
        let l_s = self.luminescence[self.config.index(x, ys)];

        let gx = (l_e - l_w) / (2.0 * self.config.dx);
        let gy = (l_n - l_s) / (2.0 * self.config.dy);
        (gx, gy)
    }
}
