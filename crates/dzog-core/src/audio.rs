//! Offline probe history sonification and WAV export for Development Gate 3.

use std::path::Path;
use hound::{WavSpec, WavWriter};
use crate::probe::PressureProbe;

/// Audio export configuration for sonification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioExportConfig {
    /// Audio sample rate in Hz (default 48000).
    pub sample_rate: u32,
    /// Simulation seconds per playback audio second (default 1.0).
    pub time_scale_a: f32,
    /// Peak output gain factor before limiting.
    pub gain: f32,
}

impl Default for AudioExportConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            time_scale_a: 1.0,
            gain: 1.0,
        }
    }
}

/// Resample probe history into audio frames at the given sample rate.
pub fn resample_probe_to_audio(
    probe: &PressureProbe,
    config: &AudioExportConfig,
) -> Vec<f32> {
    if probe.history.is_empty() {
        return Vec::new();
    }

    let t_start = probe.history.first().unwrap().time;
    let t_end = probe.history.last().unwrap().time;
    let sim_duration = (t_end - t_start).max(0.0);
    let audio_duration = sim_duration / config.time_scale_a;
    let total_audio_samples = (audio_duration * config.sample_rate as f32).ceil() as usize;

    if total_audio_samples == 0 {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(total_audio_samples);
    let mut hist_idx = 0;
    let hist_len = probe.history.len();

    for i in 0..total_audio_samples {
        let audio_t = i as f32 / config.sample_rate as f32;
        let sim_t = t_start + audio_t * config.time_scale_a;

        // Advance history pointer to bracket sim_t
        while hist_idx + 1 < hist_len && probe.history[hist_idx + 1].time <= sim_t {
            hist_idx += 1;
        }

        let sample_val = if hist_idx + 1 < hist_len {
            let t0 = probe.history[hist_idx].time;
            let t1 = probe.history[hist_idx + 1].time;
            let p0 = probe.history[hist_idx].pressure;
            let p1 = probe.history[hist_idx + 1].pressure;
            let alpha = if (t1 - t0).abs() > 1e-9 {
                ((sim_t - t0) / (t1 - t0)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            p0 + alpha * (p1 - p0)
        } else {
            probe.history[hist_idx].pressure
        };

        // Apply gain and hyperbolic tangent soft limiting to protect output
        let limited = (sample_val * config.gain).tanh();
        output.push(limited);
    }

    output
}

/// Write mono or stereo audio streams to a standard 16-bit PCM WAV file.
pub fn write_stereo_wav<P: AsRef<Path>>(
    path: P,
    sample_rate: u32,
    left_channel: &[f32],
    right_channel: &[f32],
) -> Result<(), hound::Error> {
    let spec = WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)?;
    let count = left_channel.len().max(right_channel.len());

    for i in 0..count {
        let l = left_channel.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
        let r = right_channel.get(i).copied().unwrap_or(0.0).clamp(-1.0, 1.0);

        let i16_l = (l * 32767.0) as i16;
        let i16_r = (r * 32767.0) as i16;

        writer.write_sample(i16_l)?;
        writer.write_sample(i16_r)?;
    }

    writer.finalize()?;
    Ok(())
}
