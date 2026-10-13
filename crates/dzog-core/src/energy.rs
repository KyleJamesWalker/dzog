//! Acoustic field Hamiltonian energy accounting and conservation validation.

use serde::{Deserialize, Serialize};
use crate::grid::AcousticState;
use crate::material::MaterialField;

/// Summary of acoustic field energy components at a given time step.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FieldEnergy {
    /// Compressional potential energy: integral(p^2 / (2 * K)).
    pub potential: f32,
    /// Kinetic particle velocity energy: integral(rho * v^2 / 2).
    pub kinetic: f32,
    /// Total acoustic energy = potential + kinetic.
    pub total: f32,
}

impl FieldEnergy {
    /// Calculate current energy from the state and material fields.
    pub fn compute(state: &AcousticState, material: &MaterialField) -> Self {
        let w = state.config.width;
        let h = state.config.height;
        let cell_area = state.config.dx * state.config.dy;

        let mut potential_sum = 0.0f64;
        let mut kinetic_sum = 0.0f64;

        for y in 0..h {
            for x in 0..w {
                let idx = state.config.index(x, y);

                // Potential energy: p^2 / (2 * K)
                let p = state.pressure[idx] as f64;
                let k = material.bulk_modulus[idx] as f64;
                potential_sum += (p * p) / (2.0 * k);

                // Kinetic energy on East face: rho_east * vx^2 / 2
                let vx = state.vx[idx] as f64;
                let rho_x = material.face_density_east(x, y) as f64;
                kinetic_sum += 0.5 * rho_x * vx * vx;

                // Kinetic energy on North face: rho_north * vy^2 / 2
                let vy = state.vy[idx] as f64;
                let rho_y = material.face_density_north(x, y) as f64;
                kinetic_sum += 0.5 * rho_y * vy * vy;
            }
        }

        let potential = (potential_sum * cell_area as f64) as f32;
        let kinetic = (kinetic_sum * cell_area as f64) as f32;
        let total = potential + kinetic;

        Self { potential, kinetic, total }
    }

    /// Relative energy drift compared to reference energy: |E(t) - E_0| / E_0.
    pub fn relative_drift(&self, initial_energy: &FieldEnergy) -> f32 {
        if initial_energy.total <= 1e-12 {
            return 0.0;
        }
        ((self.total - initial_energy.total).abs()) / initial_energy.total
    }
}
