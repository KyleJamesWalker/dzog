//! Spatially staggered 2D finite-difference grid layout.
//!
//! Following Dzoglang's acoustic discretization:
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
        }
    }

    /// Clears all fields to zero.
    pub fn clear(&mut self) {
        self.pressure.fill(0.0);
        self.vx.fill(0.0);
        self.vy.fill(0.0);
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
}
