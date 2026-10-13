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

impl GaussianImpulse {
    /// Inject the pressure impulse with optional toroidal periodic wrapping.
    pub fn inject_wrapped(&self, state: &mut AcousticState, periodic: bool) {
        if !periodic {
            self.inject(state);
            return;
        }
        let w = state.config.width as isize;
        let h = state.config.height as isize;
        let two_sigma_sq = 2.0 * self.sigma * self.sigma;
        let radius = (4.0 * self.sigma).ceil() as isize;
        let cx = self.center_x.round() as isize;
        let cy = self.center_y.round() as isize;

        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let px = cx + ox;
                let py = cy + oy;
                let dx = px as f32 - self.center_x;
                let dy = py as f32 - self.center_y;
                let dist_sq = dx * dx + dy * dy;
                let val = self.amplitude * (-dist_sq / two_sigma_sq).exp();
                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;
                let idx = state.config.index(gx, gy);
                state.pressure[idx] += val;
            }
        }
    }
}

impl DirectionalWavePacket {
    /// Inject the directional wave packet with optional toroidal periodic wrapping.
    pub fn inject_wrapped(&mut self, state: &mut AcousticState, material: &MaterialField, periodic: bool) {
        if !periodic {
            self.inject(state, material);
            return;
        }
        let w = state.config.width as isize;
        let h = state.config.height as isize;
        let two_sigma_sq = 2.0 * self.sigma * self.sigma;
        let cos_a = self.angle_rad.cos();
        let sin_a = self.angle_rad.sin();
        let radius = (4.0 * self.sigma).ceil() as isize;
        let cx = self.center_x.round() as isize;
        let cy = self.center_y.round() as isize;

        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let px = cx + ox;
                let py = cy + oy;
                let dx = px as f32 - self.center_x;
                let dy = py as f32 - self.center_y;
                let dist_sq = dx * dx + dy * dy;
                let p_val = self.amplitude * (-dist_sq / two_sigma_sq).exp();
                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;
                let idx = state.config.index(gx, gy);

                state.pressure[idx] += p_val;

                let z_x = (material.face_density_east_periodic(gx, gy) * material.bulk_modulus[idx]).sqrt();
                if z_x > 0.0 {
                    state.vx[idx] += (p_val / z_x) * cos_a;
                }

                let z_y = (material.face_density_north_periodic(gx, gy) * material.bulk_modulus[idx]).sqrt();
                if z_y > 0.0 {
                    state.vy[idx] += (p_val / z_y) * sin_a;
                }
            }
        }
    }
}

/// Reaction thruster plume excitation.
///
/// Injects directional momentum flux into the lattice along `exhaust_angle_rad` and returns the
/// equal-and-opposite reaction thrust impulse `(recoil_fx, recoil_fy)` imparted to the vessel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReactionThrusterPlume {
    pub nozzle_x: f32,
    pub nozzle_y: f32,
    pub sigma: f32,
    pub amplitude: f32,
    pub exhaust_angle_rad: f32,
}

impl ReactionThrusterPlume {
    pub fn new(nozzle_x: f32, nozzle_y: f32, sigma: f32, amplitude: f32, exhaust_angle_rad: f32) -> Self {
        Self {
            nozzle_x,
            nozzle_y,
            sigma,
            amplitude,
            exhaust_angle_rad,
        }
    }

    /// Inject exhaust plume into the lattice and return reaction recoil vector `(fx, fy)` on the ship.
    pub fn inject(
        &self,
        state: &mut AcousticState,
        material: &MaterialField,
        periodic: bool,
    ) -> (f32, f32) {
        let mut packet = DirectionalWavePacket::new(
            self.nozzle_x,
            self.nozzle_y,
            self.sigma,
            self.amplitude,
            self.exhaust_angle_rad,
        );
        packet.inject_wrapped(state, material, periodic);

        // Integrated reaction momentum opposite to exhaust direction
        let momentum_mag = self.amplitude * self.sigma * self.sigma * PI;
        let recoil_fx = -self.exhaust_angle_rad.cos() * momentum_mag;
        let recoil_fy = -self.exhaust_angle_rad.sin() * momentum_mag;
        (recoil_fx, recoil_fy)
    }
}

/// Phase-tuned destructive interference shield pulse (`philosophy.md` §Wave-Mediated Agency).
///
/// Samples local incoming wave excitations in an annular zone around `(center_x, center_y)` and
/// injects an antiphase response (`delta_phi = pi`) that cancels incident wave-packets while
/// harvesting the annihilated Hamiltonian field energy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AntiphaseShieldPulse {
    pub center_x: f32,
    pub center_y: f32,
    pub inner_radius: f32,
    pub outer_radius: f32,
    /// Cancellation efficiency in `[0.0, 1.0]` (e.g. `0.88` cancels 88% of incident wave amplitude).
    pub cancellation_gain: f32,
}

impl AntiphaseShieldPulse {
    pub fn new(center_x: f32, center_y: f32, inner_radius: f32, outer_radius: f32, cancellation_gain: f32) -> Self {
        Self {
            center_x,
            center_y,
            inner_radius: inner_radius.max(0.0),
            outer_radius: outer_radius.max(inner_radius + 0.5),
            cancellation_gain: cancellation_gain.clamp(0.0, 1.0),
        }
    }

    /// Apply antiphase destructive wave interference and return harvested Hamiltonian energy.
    pub fn cancel_and_harvest(
        &self,
        state: &mut AcousticState,
        material: &MaterialField,
        periodic: bool,
    ) -> f32 {
        let w = state.config.width as isize;
        let h = state.config.height as isize;
        let r_max = self.outer_radius.ceil() as isize;
        let r_in_sq = self.inner_radius * self.inner_radius;
        let r_out_sq = self.outer_radius * self.outer_radius;
        let cx = self.center_x.round() as isize;
        let cy = self.center_y.round() as isize;
        let cell_area = state.config.dx * state.config.dy;

        let mut harvested_energy = 0.0f32;

        for oy in -r_max..=r_max {
            for ox in -r_max..=r_max {
                let px = cx + ox;
                let py = cy + oy;
                if !periodic && (px < 0 || px >= w || py < 0 || py >= h) {
                    continue;
                }
                let dx = px as f32 - self.center_x;
                let dy = py as f32 - self.center_y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq < r_in_sq || dist_sq > r_out_sq {
                    continue;
                }

                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;
                let idx = state.config.index(gx, gy);

                let p_old = state.pressure[idx];
                let vx_old = state.vx[idx];
                let vy_old = state.vy[idx];
                let k = material.bulk_modulus[idx];
                let rho = material.density[idx];

                let e_before = 0.5 * (p_old * p_old / k + rho * (vx_old * vx_old + vy_old * vy_old)) * cell_area;

                // Inject antiphase wave superposition (destructive interference)
                let remain = 1.0 - self.cancellation_gain;
                state.pressure[idx] = p_old * remain;
                state.vx[idx] = vx_old * remain;
                state.vy[idx] = vy_old * remain;

                let p_new = state.pressure[idx];
                let vx_new = state.vx[idx];
                let vy_new = state.vy[idx];
                let e_after = 0.5 * (p_new * p_new / k + rho * (vx_new * vx_new + vy_new * vy_new)) * cell_area;

                harvested_energy += (e_before - e_after).max(0.0);
            }
        }

        harvested_energy
    }
}
