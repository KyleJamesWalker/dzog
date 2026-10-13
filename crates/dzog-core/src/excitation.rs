//! Field excitations: spatially smooth bounded pressure impulses and directional wave packets.

use std::f32::consts::PI;
use serde::{Deserialize, Serialize};
use crate::grid::AcousticState;
use crate::material::MaterialField;

/// Smooth, bounded Gaussian pressure impulse.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GaussianImpulse {
    /// Center X coordinate (grid units).
    pub center_x: f32,
    /// Center Y coordinate (grid units).
    pub center_y: f32,
    /// Spatial standard deviation (grid units).
    pub sigma: f32,
    /// Peak pressure amplitude.
    pub amplitude: f32,
}

impl GaussianImpulse {
    pub fn new(center_x: f32, center_y: f32, sigma: f32, amplitude: f32) -> Self {
        assert!(sigma > 0.0, "Sigma must be positive");
        assert!(amplitude.is_finite(), "Amplitude must be finite");
        Self { center_x, center_y, sigma, amplitude }
    }

    /// Inject the pressure impulse into the given acoustic state.
    pub fn inject(&self, state: &mut AcousticState) {
        let w = state.config.width;
        let h = state.config.height;
        let two_sigma_sq = 2.0 * self.sigma * self.sigma;

        // Bounded cutoff at 4 standard deviations
        let radius = (4.0 * self.sigma).ceil() as isize;
        let cx = self.center_x.round() as isize;
        let cy = self.center_y.round() as isize;

        let x_min = (cx - radius).max(0) as usize;
        let x_max = (cx + radius).min(w as isize - 1) as usize;
        let y_min = (cy - radius).max(0) as usize;
        let y_max = (cy + radius).min(h as isize - 1) as usize;

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as f32 - self.center_x;
                let dy = y as f32 - self.center_y;
                let dist_sq = dx * dx + dy * dy;
                let val = self.amplitude * (-dist_sq / two_sigma_sq).exp();
                let idx = state.config.index(x, y);
                state.pressure[idx] += val;
            }
        }
    }
}

/// Directional wave packet excitation.
///
/// By simultaneously setting pressure p and particle velocity v = (p / Z) * direction,
/// the excitation propagates unidirectionally with minimal backward reflection.
/// This behaves as a clean, coherent wave packet traveling at wave speed c.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectionalWavePacket {
    pub center_x: f32,
    pub center_y: f32,
    pub sigma: f32,
    pub amplitude: f32,
    /// Angle of propagation in radians (0 = rightwards +X, PI/2 = upwards +Y).
    pub angle_rad: f32,
}

impl DirectionalWavePacket {
    pub fn new(center_x: f32, center_y: f32, sigma: f32, amplitude: f32, angle_rad: f32) -> Self {
        assert!(sigma > 0.0);
        Self { center_x, center_y, sigma, amplitude, angle_rad }
    }

    /// Inject the directional wave packet into the field state, matching impedance Z.
    pub fn inject(&mut self, state: &mut AcousticState, material: &MaterialField) {
        let w = state.config.width;
        let h = state.config.height;
        let two_sigma_sq = 2.0 * self.sigma * self.sigma;
        let cos_a = self.angle_rad.cos();
        let sin_a = self.angle_rad.sin();

        let radius = (4.0 * self.sigma).ceil() as isize;
        let cx = self.center_x.round() as isize;
        let cy = self.center_y.round() as isize;

        let x_min = (cx - radius).max(0) as usize;
        let x_max = (cx + radius).min(w as isize - 1) as usize;
        let y_min = (cy - radius).max(0) as usize;
        let y_max = (cy + radius).min(h as isize - 1) as usize;

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as f32 - self.center_x;
                let dy = y as f32 - self.center_y;
                let dist_sq = dx * dx + dy * dy;
                let p_val = self.amplitude * (-dist_sq / two_sigma_sq).exp();
                let idx = state.config.index(x, y);

                state.pressure[idx] += p_val;

                // Match horizontal velocity on east face
                let z_x = (material.face_density_east(x, y) * material.bulk_modulus[idx]).sqrt();
                if z_x > 0.0 {
                    state.vx[idx] += (p_val / z_x) * cos_a;
                }

                // Match vertical velocity on north face
                let z_y = (material.face_density_north(x, y) * material.bulk_modulus[idx]).sqrt();
                if z_y > 0.0 {
                    state.vy[idx] += (p_val / z_y) * sin_a;
                }
            }
        }
    }
}

/// Ricker wavelet time function (often used for seismic/acoustic source testing).
pub fn ricker_wavelet(t: f32, f0: f32, t0: f32) -> f32 {
    let tau = PI * f0 * (t - t0);
    (1.0 - 2.0 * tau * tau) * (-tau * tau).exp()
}
