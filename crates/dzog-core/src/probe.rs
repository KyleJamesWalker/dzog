//! Listener and sensor probes for localized field sampling and event detection.

use serde::{Deserialize, Serialize};
use crate::grid::AcousticState;

/// Single timestamped pressure sample recorded by a probe.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ProbeSample {
    pub step: usize,
    pub time: f32,
    pub pressure: f32,
}

/// A spatial probe sampling local acoustic pressure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PressureProbe {
    pub id: String,
    pub x: usize,
    pub y: usize,
    /// Neighborhood radius for spatial smoothing (0 = single cell center).
    pub kernel_radius: usize,
    /// Recorded pressure samples.
    pub history: Vec<ProbeSample>,
    /// Optional event threshold (e.g. for goal detection in Pong).
    pub event_threshold: Option<f32>,
}

impl PressureProbe {
    pub fn new(id: impl Into<String>, x: usize, y: usize, kernel_radius: usize) -> Self {
        Self {
            id: id.into(),
            x,
            y,
            kernel_radius,
            history: Vec::new(),
            event_threshold: None,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.event_threshold = Some(threshold);
        self
    }

    /// Sample the local pressure with spatial weighted averaging.
    pub fn sample(&mut self, state: &AcousticState, step: usize, time: f32) -> f32 {
        let w = state.config.width;
        let h = state.config.height;

        let val = if self.kernel_radius == 0 {
            let idx = state.config.index(self.x.min(w - 1), self.y.min(h - 1));
            state.pressure[idx]
        } else {
            let r = self.kernel_radius as isize;
            let cx = self.x as isize;
            let cy = self.y as isize;
            let mut sum = 0.0;
            let mut weight_sum = 0.0;

            for dy in -r..=r {
                for dx in -r..=r {
                    let nx = cx + dx;
                    let ny = cy + dy;
                    if nx >= 0 && nx < w as isize && ny >= 0 && ny < h as isize {
                        let dist_sq = (dx * dx + dy * dy) as f32;
                        let weight = (-dist_sq / (2.0 * (r as f32) * (r as f32))).exp();
                        let idx = state.config.index(nx as usize, ny as usize);
                        sum += state.pressure[idx] * weight;
                        weight_sum += weight;
                    }
                }
            }

            if weight_sum > 0.0 { sum / weight_sum } else { 0.0 }
        };

        self.history.push(ProbeSample { step, time, pressure: val });
        val
    }

    /// Check if the latest sampled pressure exceeds the configured event threshold.
    pub fn is_triggered(&self) -> bool {
        if let (Some(thresh), Some(last)) = (self.event_threshold, self.history.last()) {
            last.pressure.abs() >= thresh
        } else {
            false
        }
    }

    /// Clear probe recording history.
    pub fn clear(&mut self) {
        self.history.clear();
    }
}
