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
                if idx < state.luminescence.len() {
                    state.luminescence[idx] = (state.luminescence[idx] + val.abs() * 0.45).min(4.0);
                }
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
                if idx < state.luminescence.len() {
                    state.luminescence[idx] = (state.luminescence[idx] + p_val.abs() * 0.55).min(4.0);
                }

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
                if idx < state.luminescence.len() {
                    state.luminescence[idx] = (state.luminescence[idx] + val.abs() * 0.45).min(4.0);
                }
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
                if idx < state.luminescence.len() {
                    state.luminescence[idx] = (state.luminescence[idx] + p_val.abs() * 0.55).min(4.0);
                }

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
                if idx < state.luminescence.len() {
                    state.luminescence[idx] *= remain;
                }

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

/// Continuous Soliton Carrier Coupling (`philosophy.md` §Bohm-Hiley Active Information).
///
/// Phase-locks a traveling solitary wave bolt to the staggered lattice `(p, vx, vy, luminescence)`
/// along its flight path so it continuously sustains a Bohmian quantum potential well `Q`,
/// exerts grazing radiation pressure `-grad(p)` on nearby shards, and radiates a field-native
/// exciton luminescence corona `L(x, y)`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SolitonCarrierPulse {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub sigma: f32,
    pub carrier_amplitude: f32,
    pub luminescence_gain: f32,
}

impl SolitonCarrierPulse {
    pub fn new(
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        sigma: f32,
        carrier_amplitude: f32,
        luminescence_gain: f32,
    ) -> Self {
        Self {
            x,
            y,
            vx,
            vy,
            sigma: sigma.max(0.5),
            carrier_amplitude,
            luminescence_gain,
        }
    }

    /// Coupled carrier injection into the lattice with saturation bound so energy stays finite.
    pub fn couple_to_lattice(
        &self,
        state: &mut AcousticState,
        material: &MaterialField,
        periodic: bool,
    ) {
        let w = state.config.width as isize;
        let h = state.config.height as isize;
        let two_sigma_sq = 2.0 * self.sigma * self.sigma;
        let speed = (self.vx * self.vx + self.vy * self.vy).sqrt().max(0.001);
        let dir_x = self.vx / speed;
        let dir_y = self.vy / speed;

        let radius = (3.0 * self.sigma).ceil() as isize;
        let cx = self.x.round() as isize;
        let cy = self.y.round() as isize;
        let has_lum = state.luminescence.len() == (w as usize) * (h as usize);

        for oy in -radius..=radius {
            for ox in -radius..=radius {
                let px = cx + ox;
                let py = cy + oy;
                if !periodic && (px < 0 || px >= w || py < 0 || py >= h) {
                    continue;
                }
                let dx = px as f32 - self.x;
                let dy = py as f32 - self.y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > 9.0 * self.sigma * self.sigma {
                    continue;
                }

                let env = (-dist_sq / two_sigma_sq).exp();
                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;
                let idx = state.config.index(gx, gy);

                let p_target = self.carrier_amplitude * env;
                // Bounded phase-lock: nudge local pressure toward carrier envelope without unbounded blowup
                if state.pressure[idx] < p_target {
                    state.pressure[idx] += (p_target - state.pressure[idx]) * 0.42;
                }

                let z = (material.density[idx] * material.bulk_modulus[idx]).sqrt().max(0.1);
                let v_target_x = (p_target / z) * dir_x;
                let v_target_y = (p_target / z) * dir_y;
                state.vx[idx] += (v_target_x - state.vx[idx]) * 0.30 * env;
                state.vy[idx] += (v_target_y - state.vy[idx]) * 0.30 * env;

                if has_lum {
                    let l_boost = self.luminescence_gain * env;
                    state.luminescence[idx] = (state.luminescence[idx] + l_boost).min(4.0);
                }
            }
        }
    }
}

/// Chladni Pilot-Wave Swarm Carrier (`philosophy.md` §Bohm-Hiley Active Information).
///
/// Radiates a multi-lobed standing-wave interference lattice around a matriarch cruiser.
/// Swarm drones and piezo power-cubes experience no scripted spline choreography; they are
/// organized by the resulting Bohmian quantum potential minima (`-grad Q`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChladniCarrierEmitter {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub radial_k: f32,
    pub angular_m: f32,
    pub phase: f32,
    pub amplitude: f32,
}

impl ChladniCarrierEmitter {
    pub fn new(
        center_x: f32,
        center_y: f32,
        radius: f32,
        radial_k: f32,
        angular_m: f32,
        phase: f32,
        amplitude: f32,
    ) -> Self {
        Self {
            center_x,
            center_y,
            radius: radius.max(3.0),
            radial_k,
            angular_m,
            phase,
            amplitude,
        }
    }

    /// Inject a bounded Chladni standing-wave carrier into the lattice.
    pub fn inject(&self, state: &mut AcousticState, periodic: bool) {
        let w = state.config.width as isize;
        let h = state.config.height as isize;
        let r_bound = self.radius.ceil() as isize;
        let r_sq_max = self.radius * self.radius;
        let cx = self.center_x.round() as isize;
        let cy = self.center_y.round() as isize;
        let has_lum = state.luminescence.len() == (w as usize) * (h as usize);

        for oy in -r_bound..=r_bound {
            for ox in -r_bound..=r_bound {
                let px = cx + ox;
                let py = cy + oy;
                if !periodic && (px < 1 || px >= w - 1 || py < 1 || py >= h - 1) {
                    continue;
                }
                let dx = px as f32 - self.center_x;
                let dy = py as f32 - self.center_y;
                let r_sq = dx * dx + dy * dy;
                if r_sq > r_sq_max || r_sq < 2.25 {
                    continue;
                }

                let r = r_sq.sqrt();
                let theta = dy.atan2(dx);
                let env = (1.0 - r / self.radius).powi(2);
                let mode = (self.radial_k * r - self.phase).cos() * (self.angular_m * theta).cos();
                let p_drive = self.amplitude * env * mode;

                let gx = px.rem_euclid(w) as usize;
                let gy = py.rem_euclid(h) as usize;
                let idx = state.config.index(gx, gy);
                state.pressure[idx] = (state.pressure[idx] + p_drive).clamp(-3.5, 3.5);
                if has_lum {
                    state.luminescence[idx] = (state.luminescence[idx] + p_drive.abs() * 0.22).min(4.0);
                }
            }
        }
    }
}
