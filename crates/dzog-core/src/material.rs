//! Heterogeneous acoustic material properties and interface staggered derivations.

use serde::{Deserialize, Serialize};
use crate::grid::GridConfig;

/// Acoustic material parameter field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialField {
    pub config: GridConfig,
    /// Mass density rho(x, y) > 0 at cell centers.
    pub density: Vec<f32>,
    /// Bulk modulus K(x, y) > 0 at cell centers.
    pub bulk_modulus: Vec<f32>,
    /// Dissipative loss rate gamma(x, y) >= 0 at cell centers.
    pub damping: Vec<f32>,
    /// Continuous structural cohesion C(x, y) in [0.0, 1.0] (0.0 = ambient fluid, 1.0 = intact solid).
    #[serde(default)]
    pub cohesion: Vec<f32>,
    /// Critical acoustic pressure threshold |p| above which overpressure erodes cohesion.
    #[serde(default)]
    pub yield_pressure: Vec<f32>,
    /// Topographic surface elevation field h(x, y) in [0.0, 1.0] for 2.5D slope guidance (-grad h)
    /// and single-pass horizon shadow propagation.
    #[serde(default)]
    pub elevation: Vec<f32>,
    /// Propagated solar shadow horizon h_shadow(x, y) >= h(x, y); cells with h_shadow > h are in shadow.
    #[serde(default)]
    pub shadow_horizon: Vec<f32>,
}

impl MaterialField {
    /// Create a homogeneous material field.
    pub fn homogeneous(config: GridConfig, rho: f32, bulk_modulus: f32, gamma: f32) -> Self {
        assert!(rho > 0.0 && rho.is_finite(), "Density must be positive and finite");
        assert!(bulk_modulus > 0.0 && bulk_modulus.is_finite(), "Bulk modulus must be positive and finite");
        assert!(gamma >= 0.0 && gamma.is_finite(), "Damping must be non-negative and finite");

        let count = config.cell_count();
        Self {
            config,
            density: vec![rho; count],
            bulk_modulus: vec![bulk_modulus; count],
            damping: vec![gamma; count],
            cohesion: vec![0.0; count],
            yield_pressure: vec![f32::INFINITY; count],
            elevation: vec![0.0; count],
            shadow_horizon: vec![0.0; count],
        }
    }

    /// Local acoustic wave speed c(x, y) = sqrt(K / rho).
    #[inline]
    pub fn wave_speed_at(&self, x: usize, y: usize) -> f32 {
        let idx = self.config.index(x, y);
        (self.bulk_modulus[idx] / self.density[idx]).sqrt()
    }

    /// Local acoustic characteristic impedance Z(x, y) = rho * c = sqrt(rho * K).
    #[inline]
    pub fn impedance_at(&self, x: usize, y: usize) -> f32 {
        let idx = self.config.index(x, y);
        (self.density[idx] * self.bulk_modulus[idx]).sqrt()
    }

    /// Maximum wave speed across the entire material grid: c_bound = sqrt(K_max / rho_min).
    pub fn max_wave_speed_bound(&self) -> f32 {
        let mut k_max = 0.0f32;
        let mut rho_min = f32::INFINITY;
        for (&k, &rho) in self.bulk_modulus.iter().zip(self.density.iter()) {
            if k > k_max { k_max = k; }
            if rho < rho_min { rho_min = rho; }
        }
        (k_max / rho_min).sqrt()
    }

    /// Staggered East-face effective density between (x, y) and (x + 1, y).
    /// Uses arithmetic mean of adjacent center densities: rho_face = (rho_left + rho_right) / 2.
    #[inline]
    pub fn face_density_east(&self, x: usize, y: usize) -> f32 {
        let idx_l = self.config.index(x, y);
        if x + 1 < self.config.width {
            let idx_r = self.config.index(x + 1, y);
            0.5 * (self.density[idx_l] + self.density[idx_r])
        } else {
            self.density[idx_l]
        }
    }

    /// Staggered North-face effective density between (x, y) and (x, y + 1).
    #[inline]
    pub fn face_density_north(&self, x: usize, y: usize) -> f32 {
        let idx_b = self.config.index(x, y);
        if y + 1 < self.config.height {
            let idx_t = self.config.index(x, y + 1);
            0.5 * (self.density[idx_b] + self.density[idx_t])
        } else {
            self.density[idx_b]
        }
    }

    /// Staggered East-face damping loss factor between (x, y) and (x + 1, y).
    #[inline]
    pub fn face_damping_east(&self, x: usize, y: usize) -> f32 {
        let idx_l = self.config.index(x, y);
        if x + 1 < self.config.width {
            let idx_r = self.config.index(x + 1, y);
            0.5 * (self.damping[idx_l] + self.damping[idx_r])
        } else {
            self.damping[idx_l]
        }
    }

    /// Staggered North-face damping loss factor between (x, y) and (x, y + 1).
    #[inline]
    pub fn face_damping_north(&self, x: usize, y: usize) -> f32 {
        let idx_b = self.config.index(x, y);
        if y + 1 < self.config.height {
            let idx_t = self.config.index(x, y + 1);
            0.5 * (self.damping[idx_b] + self.damping[idx_t])
        } else {
            self.damping[idx_b]
        }
    }

    /// Periodic East-face effective density between (x, y) and ((x + 1) % W, y).
    #[inline]
    pub fn face_density_east_periodic(&self, x: usize, y: usize) -> f32 {
        let idx_l = self.config.index(x, y);
        let idx_r = self.config.index((x + 1) % self.config.width, y);
        0.5 * (self.density[idx_l] + self.density[idx_r])
    }

    /// Periodic North-face effective density between (x, y) and (x, (y + 1) % H).
    #[inline]
    pub fn face_density_north_periodic(&self, x: usize, y: usize) -> f32 {
        let idx_b = self.config.index(x, y);
        let idx_t = self.config.index(x, (y + 1) % self.config.height);
        0.5 * (self.density[idx_b] + self.density[idx_t])
    }

    /// Periodic East-face damping loss factor between (x, y) and ((x + 1) % W, y).
    #[inline]
    pub fn face_damping_east_periodic(&self, x: usize, y: usize) -> f32 {
        let idx_l = self.config.index(x, y);
        let idx_r = self.config.index((x + 1) % self.config.width, y);
        0.5 * (self.damping[idx_l] + self.damping[idx_r])
    }

    /// Periodic North-face damping loss factor between (x, y) and (x, (y + 1) % H).
    #[inline]
    pub fn face_damping_north_periodic(&self, x: usize, y: usize) -> f32 {
        let idx_b = self.config.index(x, y);
        let idx_t = self.config.index(x, (y + 1) % self.config.height);
        0.5 * (self.damping[idx_b] + self.damping[idx_t])
    }

    /// Paint a circular or rectangular material modification region.
    pub fn set_region(
        &mut self,
        x_min: usize,
        x_max: usize,
        y_min: usize,
        y_max: usize,
        rho: f32,
        k: f32,
        gamma: f32,
    ) {
        assert!(rho > 0.0 && k > 0.0 && gamma >= 0.0);
        let x_end = x_max.min(self.config.width);
        let y_end = y_max.min(self.config.height);

        for y in y_min..y_end {
            for x in x_min..x_end {
                let idx = self.config.index(x, y);
                self.density[idx] = rho;
                self.bulk_modulus[idx] = k;
                self.damping[idx] = gamma;
            }
        }
    }

    /// Paint a structured material region with explicit cohesion C in [0, 1] and yield pressure.
    #[allow(clippy::too_many_arguments)]
    pub fn set_cohesive_region(
        &mut self,
        x_min: usize,
        x_max: usize,
        y_min: usize,
        y_max: usize,
        rho: f32,
        k: f32,
        gamma: f32,
        cohesion: f32,
        yield_pressure: f32,
    ) {
        assert!(rho > 0.0 && k > 0.0 && gamma >= 0.0);
        let x_end = x_max.min(self.config.width);
        let y_end = y_max.min(self.config.height);
        let c_clamped = cohesion.clamp(0.0, 1.0);

        for y in y_min..y_end {
            for x in x_min..x_end {
                let idx = self.config.index(x, y);
                self.density[idx] = rho;
                self.bulk_modulus[idx] = k;
                self.damping[idx] = gamma;
                self.cohesion[idx] = c_clamped;
                self.yield_pressure[idx] = yield_pressure;
            }
        }
    }

    /// Apply an absorbing sponge boundary layer of thickness `sponge_cells`.
    /// Increases damping quadratically towards outer borders.
    pub fn apply_sponge_layer(&mut self, sponge_cells: usize, max_gamma: f32) {
        if sponge_cells == 0 {
            return;
        }
        let w = self.config.width;
        let h = self.config.height;

        for y in 0..h {
            for x in 0..w {
                let dist_x = x.min(w - 1 - x);
                let dist_y = y.min(h - 1 - y);
                let min_dist = dist_x.min(dist_y);

                if min_dist < sponge_cells {
                    let factor = (sponge_cells - min_dist) as f32 / sponge_cells as f32;
                    let sponge_gamma = max_gamma * factor * factor;
                    let idx = self.config.index(x, y);
                    self.damping[idx] = self.damping[idx].max(sponge_gamma);
                }
            }
        }
    }

    /// Validate material parameter bounds.
    pub fn validate(&self) -> Result<(), String> {
        let count = self.config.cell_count();
        if self.density.len() != count || self.bulk_modulus.len() != count || self.damping.len() != count {
            return Err("Material parameter array length mismatch".to_string());
        }
        if !self.cohesion.is_empty() && self.cohesion.len() != count {
            return Err("Material cohesion array length mismatch".to_string());
        }
        if !self.yield_pressure.is_empty() && self.yield_pressure.len() != count {
            return Err("Material yield_pressure array length mismatch".to_string());
        }
        for i in 0..count {
            let rho = self.density[i];
            let k = self.bulk_modulus[i];
            let gamma = self.damping[i];
            if !rho.is_finite() || rho <= 0.0 {
                let (x, y) = self.config.coords(i);
                return Err(format!("Invalid density rho={rho} at ({x}, {y})"));
            }
            if !k.is_finite() || k <= 0.0 {
                let (x, y) = self.config.coords(i);
                return Err(format!("Invalid bulk modulus K={k} at ({x}, {y})"));
            }
            if !gamma.is_finite() || gamma < 0.0 {
                let (x, y) = self.config.coords(i);
                return Err(format!("Invalid damping gamma={gamma} at ({x}, {y})"));
            }
        }
        Ok(())
    }

    /// Sample surface elevation `h(x, y)` and downhill slope normal `(-dh/dx, -dh/dy)` at
    /// continuous lattice coordinate `(wx, wy)` in `O(1)` time via central differences.
    #[inline]
    pub fn elevation_and_gradient_at(&self, wx: f32, wy: f32) -> (f32, f32, f32) {
        if self.elevation.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        let w = self.config.width;
        let h = self.config.height;
        let gx = (wx.round() as isize).clamp(0, w as isize - 1) as usize;
        let gy = (wy.round() as isize).clamp(0, h as isize - 1) as usize;

        let elev = self.elevation[gy * w + gx];
        let xm = gx.saturating_sub(1);
        let xp = (gx + 1).min(w - 1);
        let ym = gy.saturating_sub(1);
        let yp = (gy + 1).min(h - 1);

        let dh_dx = 0.5 * (self.elevation[gy * w + xp] - self.elevation[gy * w + xm]);
        let dh_dy = 0.5 * (self.elevation[yp * w + gx] - self.elevation[ym * w + gx]);
        let mag = (dh_dx * dh_dx + dh_dy * dh_dy + 0.16).sqrt();
        (elev, -dh_dx / mag, -dh_dy / mag)
    }

    /// Single-pass `O(cells)` dynamic-programming shadow horizon sweep along sun ray `(-1, +1)`.
    ///
    /// Every elevated terrain cliff, platform edge, dome, or turret automatically casts a
    /// height-proportional shadow into `self.shadow_horizon` without per-object raymarching loops.
    pub fn recompute_horizon_shadows(&mut self, sun_slope: f32) {
        let w = self.config.width;
        let h = self.config.height;
        let count = w * h;
        if self.elevation.len() != count {
            self.elevation.resize(count, 0.0);
        }
        if self.shadow_horizon.len() != count {
            self.shadow_horizon.resize(count, 0.0);
        }

        // Sweep from North-East (top-right) toward South-West (-X, +Y) strictly along the diagonal sun ray
        for y in 0..h {
            for x in (0..w).rev() {
                let idx = y * w + x;
                let base_h = self.elevation[idx];
                let mut horizon = base_h;
                if y > 0 && x + 1 < w {
                    let up_sun_h = self.shadow_horizon[(y - 1) * w + (x + 1)] - sun_slope;
                    if up_sun_h > horizon {
                        horizon = up_sun_h;
                    }
                }
                self.shadow_horizon[idx] = horizon;
            }
        }
    }

    /// Bilinearly sample shadow depth `max(0, h_shadow - h)` at continuous coordinate `(wx, wy)`.
    #[inline]
    pub fn shadow_depth_at(&self, wx: f32, wy: f32) -> f32 {
        let w = self.config.width;
        let h = self.config.height;
        if self.shadow_horizon.len() != w * h || self.elevation.len() != w * h {
            return 0.0;
        }
        let x0 = (wx.floor() as isize).clamp(0, w as isize - 1) as usize;
        let y0 = (wy.floor() as isize).clamp(0, h as isize - 1) as usize;
        let x1 = (x0 + 1).min(w - 1);
        let y1 = (y0 + 1).min(h - 1);
        let fx = (wx - x0 as f32).clamp(0.0, 1.0);
        let fy = (wy - y0 as f32).clamp(0.0, 1.0);

        let depth = |ix: usize, iy: usize| -> f32 {
            let i = iy * w + ix;
            (self.shadow_horizon[i] - self.elevation[i]).max(0.0)
        };
        let d0 = depth(x0, y0) * (1.0 - fx) + depth(x1, y0) * fx;
        let d1 = depth(x0, y1) * (1.0 - fx) + depth(x1, y1) * fx;
        d0 * (1.0 - fy) + d1 * fy
    }
}
