//! Boundary conditions: impermeable walls, moving boundaries (paddles), and outer edge rules.

use serde::{Deserialize, Serialize};
use crate::grid::GridConfig;

/// Outer boundary behavior condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OuterBoundaryRule {
    /// Rigid impermeable outer boundaries: normal velocity forced to zero on all 4 borders.
    RigidWall,
    /// Open boundary with an absorbing damping sponge layer.
    AbsorbingSponge,
    /// Periodic toroidal wrap in both dimensions (primarily for stencil verification).
    Periodic,
}

/// Boundary mask and moving wall state for staggered faces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryMask {
    pub config: GridConfig,
    pub outer_rule: OuterBoundaryRule,

    /// East-face wall mask: true if normal flow across the face is blocked.
    pub wall_east: Vec<bool>,
    /// Imposed normal velocity on blocked East faces (e.g. horizontal paddle velocity).
    pub wall_vx: Vec<f32>,

    /// North-face wall mask: true if normal flow across the face is blocked.
    pub wall_north: Vec<bool>,
    /// Imposed normal velocity on blocked North faces (e.g. vertical paddle velocity).
    pub wall_vy: Vec<f32>,
}

impl BoundaryMask {
    pub fn new(config: GridConfig, outer_rule: OuterBoundaryRule) -> Self {
        let count = config.cell_count();
        Self {
            config,
            outer_rule,
            wall_east: vec![false; count],
            wall_vx: vec![0.0; count],
            wall_north: vec![false; count],
            wall_vy: vec![0.0; count],
        }
    }

    /// Reset all interior walls to open (unblocked).
    pub fn clear_interior(&mut self) {
        self.wall_east.fill(false);
        self.wall_vx.fill(0.0);
        self.wall_north.fill(false);
        self.wall_vy.fill(0.0);
    }

    /// Add a stationary vertical wall segment at x (between x and x + 1) for y in [y_min, y_max].
    pub fn add_vertical_wall(&mut self, x: usize, y_min: usize, y_max: usize) {
        if x < self.config.width {
            let y_end = y_max.min(self.config.height - 1);
            for y in y_min..=y_end {
                let idx = self.config.index(x, y);
                self.wall_east[idx] = true;
                self.wall_vx[idx] = 0.0;
            }
        }
    }

    /// Add a stationary horizontal wall segment at y (between y and y + 1) for x in [x_min, x_max].
    pub fn add_horizontal_wall(&mut self, y: usize, x_min: usize, x_max: usize) {
        if y < self.config.height {
            let x_end = x_max.min(self.config.width - 1);
            for x in x_min..=x_end {
                let idx = self.config.index(x, y);
                self.wall_north[idx] = true;
                self.wall_vy[idx] = 0.0;
            }
        }
    }

    /// Set an impermeable rectangular body (like a paddle) with moving velocity (vx, vy).
    /// All bounding faces of the rectangle enforce normal velocity = (vx, vy),
    /// and internal faces are blocked.
    pub fn set_moving_rect(
        &mut self,
        x_min: usize,
        x_max: usize,
        y_min: usize,
        y_max: usize,
        vx: f32,
        vy: f32,
    ) {
        let x_end = x_max.min(self.config.width - 1);
        let y_end = y_max.min(self.config.height - 1);

        for y in y_min..=y_end {
            for x in x_min..=x_end {
                let idx = self.config.index(x, y);
                // Internal and bounding east faces
                self.wall_east[idx] = true;
                self.wall_vx[idx] = vx;

                // Internal and bounding north faces
                self.wall_north[idx] = true;
                self.wall_vy[idx] = vy;
            }
        }
    }

    /// Clear a specific rectangular region of walls.
    pub fn clear_region(&mut self, x_min: usize, x_max: usize, y_min: usize, y_max: usize) {
        let x_end = x_max.min(self.config.width - 1);
        let y_end = y_max.min(self.config.height - 1);

        for y in y_min..=y_end {
            for x in x_min..=x_end {
                let idx = self.config.index(x, y);
                self.wall_east[idx] = false;
                self.wall_vx[idx] = 0.0;
                self.wall_north[idx] = false;
                self.wall_vy[idx] = 0.0;
            }
        }
    }
}
