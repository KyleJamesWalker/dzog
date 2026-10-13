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

/// Moving binaural cockpit listener and lock-free SPSC ring buffer for real-time field sonification.
///
/// Samples port and starboard wingtip pressures `(p_left, p_right)` plus low-frequency hull
/// resonance directly from the evolving PDE lattice. Because the listener moves with the ship,
/// Doppler pitch shifts on flybys, thruster rumble, and cavitation shockwaves emerge naturally.
#[derive(Debug, Clone)]
pub struct BinauralCockpitSonifier {
    /// Wingtip separation half-span in grid cells.
    pub wing_span: f32,
    /// 1st-order DC-blocking filter state for Left channel: y[n] = x[n] - x[n-1] + R * y[n-1].
    dc_prev_in_l: f32,
    dc_prev_out_l: f32,
    /// 1st-order DC-blocking filter state for Right channel.
    dc_prev_in_r: f32,
    dc_prev_out_r: f32,
    /// Ring buffer of recent stereo frames `(left, right)` for real-time streaming / HUD telemetry.
    pub ring_frames: std::collections::VecDeque<(f32, f32)>,
    pub max_ring_frames: usize,
    /// Instantaneous RMS acoustic amplitude on Left and Right cockpit wingtips.
    pub rms_left: f32,
    pub rms_right: f32,
    /// Estimated instantaneous zero-crossing Doppler frequency (normalized cycles/substep).
    pub doppler_freq: f32,
    zero_crossings: u32,
    sample_counter: u32,
}

impl Default for BinauralCockpitSonifier {
    fn default() -> Self {
        Self::new(2.5, 4096)
    }
}

impl BinauralCockpitSonifier {
    pub fn new(wing_span: f32, max_ring_frames: usize) -> Self {
        Self {
            wing_span: wing_span.max(0.5),
            dc_prev_in_l: 0.0,
            dc_prev_out_l: 0.0,
            dc_prev_in_r: 0.0,
            dc_prev_out_r: 0.0,
            ring_frames: std::collections::VecDeque::with_capacity(max_ring_frames),
            max_ring_frames,
            rms_left: 0.0,
            rms_right: 0.0,
            doppler_freq: 0.0,
            zero_crossings: 0,
            sample_counter: 0,
        }
    }

    /// Sample port and starboard pressures at ship pose `(x, y, theta)` from the PDE state.
    pub fn sample_cockpit(
        &mut self,
        state: &crate::grid::AcousticState,
        x: f32,
        y: f32,
        theta: f32,
        periodic: bool,
    ) -> (f32, f32) {
        let w = state.config.width as isize;
        let h = state.config.height as isize;

        // Port (-perp) and Starboard (+perp) wingtip coordinates
        let perp_x = -theta.sin() * self.wing_span;
        let perp_y = theta.cos() * self.wing_span;

        let sample_at = |wx: f32, wy: f32| -> f32 {
            let ix = wx.round() as isize;
            let iy = wy.round() as isize;
            let (gx, gy) = if periodic {
                (ix.rem_euclid(w) as usize, iy.rem_euclid(h) as usize)
            } else {
                (
                    ix.clamp(0, w - 1) as usize,
                    iy.clamp(0, h - 1) as usize,
                )
            };
            state.pressure[state.config.index(gx, gy)]
        };

        let raw_l = sample_at(x - perp_x, y - perp_y);
        let raw_r = sample_at(x + perp_x, y + perp_y);

        // 1st-order DC blocker (R = 0.992) + tanh soft-clipping
        const R: f32 = 0.992;
        let out_l = (raw_l - self.dc_prev_in_l + R * self.dc_prev_out_l).tanh();
        let prev_sign = self.dc_prev_out_l >= 0.0;
        self.dc_prev_in_l = raw_l;
        self.dc_prev_out_l = out_l;

        let out_r = (raw_r - self.dc_prev_in_r + R * self.dc_prev_out_r).tanh();
        self.dc_prev_in_r = raw_r;
        self.dc_prev_out_r = out_r;

        // Track zero-crossing rate for Doppler telemetry
        if (out_l >= 0.0) != prev_sign && out_l.abs() > 0.005 {
            self.zero_crossings += 1;
        }
        self.sample_counter += 1;
        if self.sample_counter >= 32 {
            self.doppler_freq = self.zero_crossings as f32 / (2.0 * self.sample_counter as f32);
            self.zero_crossings = 0;
            self.sample_counter = 0;
        }

        // Exponential moving RMS envelope
        self.rms_left = (0.88 * self.rms_left * self.rms_left + 0.12 * out_l * out_l).sqrt();
        self.rms_right = (0.88 * self.rms_right * self.rms_right + 0.12 * out_r * out_r).sqrt();

        if self.ring_frames.len() >= self.max_ring_frames {
            self.ring_frames.pop_front();
        }
        self.ring_frames.push_back((out_l, out_r));

        (out_l, out_r)
    }

    /// Sample both local cockpit wingtips and spatialized lattice modal energies, updating
    /// HUD RMS/Doppler telemetry and returning a `FieldAcousticFrame` for real-time `cpal` synthesis.
    #[allow(clippy::too_many_arguments)]
    pub fn sample_full_field(
        &mut self,
        state: &crate::grid::AcousticState,
        material: &crate::material::MaterialField,
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        theta: f32,
        periodic: bool,
    ) -> FieldAcousticFrame {
        let (wing_l, wing_r) = self.sample_cockpit(state, x, y, theta, periodic);
        let frame = FieldAcousticFrame::from_lattice(
            state, material, x, y, vx, vy, theta, wing_l, wing_r, periodic,
        );

        let total_l = (wing_l.abs()
            + frame.vacuum_wave_l * 0.6
            + frame.quartz_mode_l * 0.8
            + frame.obsidian_mode_l * 0.8
            + frame.bismuth_alien_l * 0.8
            + frame.cavitation_shock_l)
            .tanh();
        let total_r = (wing_r.abs()
            + frame.vacuum_wave_r * 0.6
            + frame.quartz_mode_r * 0.8
            + frame.obsidian_mode_r * 0.8
            + frame.bismuth_alien_r * 0.8
            + frame.cavitation_shock_r)
            .tanh();

        self.rms_left = 0.75 * self.rms_left + 0.25 * total_l;
        self.rms_right = 0.75 * self.rms_right + 0.25 * total_r;
        self.doppler_freq = frame.doppler_factor;
        frame
    }
}

/// Instantaneous physical field acoustic state and transient excitation triggers
/// shared between the PDE simulation and the 48 kHz `FieldModalSynthesizer`.
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldAcousticFrame {
    /// Direct cockpit wingtip pressure perturbations (Left / Right) for HUD telemetry.
    pub wing_p_l: f32,
    pub wing_p_r: f32,
    /// Free-vacuum soliton wave amplitude in Port / Starboard hemispheres (HUD + test telemetry).
    pub vacuum_wave_l: f32,
    pub vacuum_wave_r: f32,
    /// PiezoQuartz crystal phonon resonance in Port / Starboard hemispheres.
    pub quartz_mode_l: f32,
    pub quartz_mode_r: f32,
    /// Obsidian Anvil heavy acoustic stress in Port / Starboard hemispheres.
    pub obsidian_mode_l: f32,
    pub obsidian_mode_r: f32,
    /// Bismuth metamaterial & Alien saucer resonance in Port / Starboard hemispheres.
    pub bismuth_alien_l: f32,
    pub bismuth_alien_r: f32,
    /// High-intensity cavitation shock amplitude.
    pub cavitation_shock_l: f32,
    pub cavitation_shock_r: f32,
    /// Relativistic/acoustic Doppler frequency multiplier `1.0 + delta_f`.
    pub doppler_factor: f32,
    /// Mean Bohmian quantum potential `|Q|` near the listener.
    pub bohmian_q_mod: f32,

    // --- Event-Driven Physical Excitation Triggers (consumed by 48 kHz audio thread) ---
    /// Player soliton laser trigger amplitude (`> 0.0` when fired; `> 1.2` when overcharged).
    pub trig_player_laser: f32,
    /// Enemy alien soliton laser trigger amplitude and stereo pan `[-1.0, 1.0]`.
    pub trig_enemy_laser: f32,
    pub trig_enemy_pan: f32,
    /// Crystalline crack / impact trigger amplitude, mineral wave speed `c`, and stereo pan.
    pub trig_crystal_crack: f32,
    pub trig_crystal_c: f32,
    pub trig_crystal_pan: f32,
    /// Asteroid topological cleave / cavitation shatter explosion trigger and stereo pan.
    pub trig_cavitation_break: f32,
    pub trig_break_pan: f32,
    /// Antiphase shield parry / bow-shock trigger.
    pub trig_shield_pulse: f32,
}

impl FieldAcousticFrame {
    /// Compute stereo pan in `[-1.0 (Port), +1.0 (Starboard)]` for a world event at `(ex, ey)`.
    pub fn compute_stereo_pan(
        ship_x: f32,
        ship_y: f32,
        ship_theta: f32,
        ex: f32,
        ey: f32,
        periodic_dims: Option<(f32, f32)>,
    ) -> f32 {
        let mut dx = ex - ship_x;
        let mut dy = ey - ship_y;
        if let Some((w, h)) = periodic_dims {
            if dx > 0.5 * w { dx -= w; } else if dx < -0.5 * w { dx += w; }
            if dy > 0.5 * h { dy -= h; } else if dy < -0.5 * h { dy += h; }
        }
        let dist = (dx * dx + dy * dy).sqrt().max(1.0);
        let right_nx = -ship_theta.sin();
        let right_ny = ship_theta.cos();
        ((dx * right_nx + dy * right_ny) / dist).clamp(-0.85, 0.85)
    }

    /// Trigger a crisp player soliton laser shot (`overcharged` boosts harmonic sweep).
    pub fn trigger_player_laser(&mut self, overcharged: bool) {
        self.trig_player_laser = if overcharged { 1.45 } else { 1.0 };
        self.vacuum_wave_l = self.vacuum_wave_l.max(0.6);
        self.vacuum_wave_r = self.vacuum_wave_r.max(0.6);
    }

    /// Trigger an enemy alien soliton laser shot panned to `pan` in `[-1.0, 1.0]`.
    pub fn trigger_enemy_laser(&mut self, pan: f32) {
        self.trig_enemy_laser = 1.0;
        self.trig_enemy_pan = pan.clamp(-0.9, 0.9);
    }

    /// Trigger a solid-state crystal crack / glancing impact tuned to mineral wave speed `c`.
    pub fn trigger_crystal_crack(&mut self, pan: f32, wave_speed_c: f32) {
        self.trig_crystal_crack = 1.1;
        self.trig_crystal_c = wave_speed_c.clamp(0.8, 1.8);
        self.trig_crystal_pan = pan.clamp(-0.9, 0.9);
    }

    /// Trigger a loud asteroid topological cleave / cavitation shatter explosion!
    pub fn trigger_cavitation_break(&mut self, pan: f32, intensity: f32) {
        self.trig_cavitation_break = self
            .trig_cavitation_break
            .max(intensity.clamp(0.9, 1.8));
        self.trig_break_pan = pan.clamp(-0.9, 0.9);
        self.cavitation_shock_l = self.cavitation_shock_l.max(intensity);
        self.cavitation_shock_r = self.cavitation_shock_r.max(intensity);
    }

    /// Trigger an antiphase shield parry or bow-brake shock pulse.
    pub fn trigger_shield_pulse(&mut self, parried: bool) {
        self.trig_shield_pulse = if parried { 1.35 } else { 0.85 };
    }

    /// Merge lattice telemetry without overwriting pending transient `trig_*` flags.
    pub fn merge_telemetry(&mut self, other: &Self) {
        self.wing_p_l = other.wing_p_l;
        self.wing_p_r = other.wing_p_r;
        self.vacuum_wave_l = other.vacuum_wave_l;
        self.vacuum_wave_r = other.vacuum_wave_r;
        self.quartz_mode_l = other.quartz_mode_l;
        self.quartz_mode_r = other.quartz_mode_r;
        self.obsidian_mode_l = other.obsidian_mode_l;
        self.obsidian_mode_r = other.obsidian_mode_r;
        self.bismuth_alien_l = other.bismuth_alien_l;
        self.bismuth_alien_r = other.bismuth_alien_r;
        self.cavitation_shock_l = other.cavitation_shock_l;
        self.cavitation_shock_r = other.cavitation_shock_r;
        self.doppler_factor = other.doppler_factor;
        self.bohmian_q_mod = other.bohmian_q_mod;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_lattice(
        state: &crate::grid::AcousticState,
        material: &crate::material::MaterialField,
        ship_x: f32,
        ship_y: f32,
        ship_vx: f32,
        ship_vy: f32,
        ship_theta: f32,
        wing_p_l: f32,
        wing_p_r: f32,
        periodic: bool,
    ) -> Self {
        let w = state.config.width;
        let h = state.config.height;
        let wf = w as f32;
        let hf = h as f32;

        let right_nx = -ship_theta.sin();
        let right_ny = ship_theta.cos();

        let mut vac_l = 0.0f32;
        let mut vac_r = 0.0f32;
        let mut qtz_l = 0.0f32;
        let mut qtz_r = 0.0f32;
        let mut obs_l = 0.0f32;
        let mut obs_r = 0.0f32;
        let mut bis_l = 0.0f32;
        let mut bis_r = 0.0f32;
        let mut cav_l = 0.0f32;
        let mut cav_r = 0.0f32;
        let mut flux_dot_ship = 0.0f32;
        let mut flux_weight = 0.01f32;

        let has_cohesion = material.cohesion.len() == w * h;
        for y in (0..h).step_by(2) {
            for x in (0..w).step_by(2) {
                let idx = state.config.index(x, y);
                let p = state.pressure[idx];
                let p_abs = p.abs();
                if p_abs < 0.15 {
                    continue;
                }

                let vx = state.vx[idx];
                let vy = state.vy[idx];
                let rho = material.density[idx];
                let coh = if has_cohesion { material.cohesion[idx] } else { 0.0 };

                let mut dx = x as f32 - ship_x;
                let mut dy = y as f32 - ship_y;
                if periodic {
                    if dx > 0.5 * wf { dx -= wf; } else if dx < -0.5 * wf { dx += w as f32; }
                    if dy > 0.5 * hf { dy -= hf; } else if dy < -0.5 * hf { dy += h as f32; }
                }
                let dist = (dx * dx + dy * dy).sqrt().max(2.0);
                let atten = (14.0 / (dist + 10.0)).clamp(0.10, 1.0);
                let pan = ((dx * right_nx + dy * right_ny) / dist).clamp(-1.0, 1.0);
                let gain_l = atten * (0.5 * (1.0 - pan)).sqrt();
                let gain_r = atten * (0.5 * (1.0 + pan)).sqrt();

                let amp = p_abs * atten;
                let rel_vx = ship_vx - vx;
                let rel_vy = ship_vy - vy;
                let radial_closing = -(dx * rel_vx + dy * rel_vy) / dist;
                flux_dot_ship += radial_closing * amp;
                flux_weight += amp;

                if coh >= 0.15 {
                    if rho < 0.90 {
                        qtz_l += p_abs * gain_l;
                        qtz_r += p_abs * gain_r;
                    } else if rho > 1.60 {
                        obs_l += p_abs * gain_l;
                        obs_r += p_abs * gain_r;
                    } else {
                        bis_l += p_abs * gain_l;
                        bis_r += p_abs * gain_r;
                    }
                } else {
                    vac_l += p_abs * gain_l;
                    vac_r += p_abs * gain_r;
                }

                if p_abs > 1.25 {
                    let excess = p_abs - 1.25;
                    cav_l += excess * gain_l;
                    cav_r += excess * gain_r;
                }
            }
        }

        let sx = (ship_x.round() as isize).rem_euclid(w as isize) as usize;
        let sy = (ship_y.round() as isize).rem_euclid(h as isize) as usize;
        let q_local = state
            .quantum_potential_wrapped_at(sx, sy, periodic)
            .abs()
            .min(8.0);

        let mean_closing = (flux_dot_ship / flux_weight).clamp(-0.65, 0.65);
        let ship_speed = (ship_vx * ship_vx + ship_vy * ship_vy).sqrt();
        let doppler_factor = (1.0 + mean_closing * 0.45 + ship_speed * 0.25).clamp(0.65, 1.65);

        Self {
            wing_p_l: if wing_p_l.abs() >= 0.15 { wing_p_l } else { 0.0 },
            wing_p_r: if wing_p_r.abs() >= 0.15 { wing_p_r } else { 0.0 },
            vacuum_wave_l: (vac_l * 0.08).min(1.5),
            vacuum_wave_r: (vac_r * 0.08).min(1.5),
            quartz_mode_l: (qtz_l * 0.18).min(1.5),
            quartz_mode_r: (qtz_r * 0.18).min(1.5),
            obsidian_mode_l: (obs_l * 0.18).min(1.5),
            obsidian_mode_r: (obs_r * 0.18).min(1.5),
            bismuth_alien_l: (bis_l * 0.16).min(1.5),
            bismuth_alien_r: (bis_r * 0.16).min(1.5),
            cavitation_shock_l: (cav_l * 0.18).min(1.5),
            cavitation_shock_r: (cav_r * 0.18).min(1.5),
            doppler_factor,
            bohmian_q_mod: q_local,
            ..Default::default()
        }
    }
}

/// Pure, host-agnostic modal audio synthesizer driven by `FieldAcousticFrame`.
///
/// Synthesizes:
/// - **Zero background hum during movement** (empty space is 100% silent).
/// - **Player Soliton Laser**: Crisp downward FM frequency sweep (`1250 Hz -> 165 Hz`).
/// - **Alien Soliton Laser**: Warbling Bohmian ring-modulated laser (`760 Hz -> 210 Hz`).
/// - **Crystal Crack / Impact**: High-Q mineralogical strike tuned to crystal wave speed `c`.
/// - **Asteroid Cleave / Cavitation Shatter**: Loud low-end shock punch (`145 Hz -> 36 Hz`) +
///   crystalline shard partials + broadband shatter roar.
pub struct FieldModalSynthesizer {
    pub shared_frame: std::sync::Arc<std::sync::Mutex<FieldAcousticFrame>>,
    pub sample_rate: u32,

    // Voice 1: Player Soliton Laser
    player_laser_env: f32,
    player_laser_freq: f32,
    player_laser_phase: f32,

    // Voice 2: Enemy Alien Laser
    enemy_laser_env: f32,
    enemy_laser_freq: f32,
    enemy_laser_phase: f32,
    enemy_mod_phase: f32,
    enemy_pan: f32,

    // Voice 3: Mineralogical Crystal Crack / Strike
    crystal_env: f32,
    crystal_freq: f32,
    crystal_phase1: f32,
    crystal_phase2: f32,
    crystal_pan: f32,

    // Voice 4: Asteroid Topological Cleave / Cavitation Explosion
    break_env: f32,
    break_sub_freq: f32,
    break_sub_phase: f32,
    break_shard_phase: f32,
    break_pan: f32,

    // Voice 5: Antiphase Shield Parry / Bow-Shock
    shield_env: f32,
    shield_freq: f32,
    shield_phase: f32,

    noise_state: u32,
    lp_noise_l: f32,
    lp_noise_r: f32,
    pending_right: Option<f32>,
}

impl FieldModalSynthesizer {
    pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

    pub fn new(shared_frame: std::sync::Arc<std::sync::Mutex<FieldAcousticFrame>>) -> Self {
        Self {
            shared_frame,
            sample_rate: Self::DEFAULT_SAMPLE_RATE,
            player_laser_env: 0.0,
            player_laser_freq: 1000.0,
            player_laser_phase: 0.0,
            enemy_laser_env: 0.0,
            enemy_laser_freq: 700.0,
            enemy_laser_phase: 0.0,
            enemy_mod_phase: 0.0,
            enemy_pan: 0.0,
            crystal_env: 0.0,
            crystal_freq: 640.0,
            crystal_phase1: 0.0,
            crystal_phase2: 0.0,
            crystal_pan: 0.0,
            break_env: 0.0,
            break_sub_freq: 140.0,
            break_sub_phase: 0.0,
            break_shard_phase: 0.0,
            break_pan: 0.0,
            shield_env: 0.0,
            shield_freq: 260.0,
            shield_phase: 0.0,
            noise_state: 0x1234_5678,
            lp_noise_l: 0.0,
            lp_noise_r: 0.0,
            pending_right: None,
        }
    }

    /// Generate the next stereo frame `(left, right)` at `self.sample_rate`.
    pub fn next_stereo_frame(&mut self) -> (f32, f32) {
        // Consume any pending physical excitation triggers from the game thread
        if let Ok(mut shared) = self.shared_frame.try_lock() {
            if shared.trig_player_laser > 0.0 {
                self.player_laser_env = shared.trig_player_laser;
                self.player_laser_freq = if shared.trig_player_laser > 1.2 {
                    1480.0
                } else {
                    1180.0
                };
                shared.trig_player_laser = 0.0;
            }
            if shared.trig_enemy_laser > 0.0 {
                self.enemy_laser_env = shared.trig_enemy_laser;
                self.enemy_laser_freq = 760.0;
                self.enemy_pan = shared.trig_enemy_pan;
                shared.trig_enemy_laser = 0.0;
            }
            if shared.trig_crystal_crack > 0.0 {
                self.crystal_env = shared.trig_crystal_crack;
                self.crystal_freq = 440.0 * shared.trig_crystal_c;
                self.crystal_pan = shared.trig_crystal_pan;
                shared.trig_crystal_crack = 0.0;
            }
            if shared.trig_cavitation_break > 0.0 {
                self.break_env = self.break_env.max(shared.trig_cavitation_break);
                self.break_sub_freq = 148.0;
                self.break_pan = shared.trig_break_pan;
                shared.trig_cavitation_break = 0.0;
            }
            if shared.trig_shield_pulse > 0.0 {
                self.shield_env = shared.trig_shield_pulse;
                self.shield_freq = 240.0;
                shared.trig_shield_pulse = 0.0;
            }
        }

        let active_sum = self.player_laser_env
            + self.enemy_laser_env
            + self.crystal_env
            + self.break_env
            + self.shield_env;

        // 100% dead-silent vacuum when no physical shots/impacts/shatters are active
        if active_sum < 0.0015 {
            self.player_laser_env = 0.0;
            self.enemy_laser_env = 0.0;
            self.crystal_env = 0.0;
            self.break_env = 0.0;
            self.shield_env = 0.0;
            return (0.0, 0.0);
        }

        let dt = 1.0 / (self.sample_rate as f32);
        let tau = std::f32::consts::TAU;
        let mut mix_l = 0.0f32;
        let mut mix_r = 0.0f32;

        // 1. Player Soliton Laser (Crisp downward FM sweep 1180 Hz -> 165 Hz over ~95 ms)
        if self.player_laser_env > 0.001 {
            self.player_laser_freq = (self.player_laser_freq * 0.99935).max(165.0);
            self.player_laser_phase =
                (self.player_laser_phase + tau * self.player_laser_freq * dt).rem_euclid(tau);
            let fm_sig = (self.player_laser_phase
                + 0.65 * (2.0 * self.player_laser_phase).sin())
            .sin();
            let s = fm_sig * self.player_laser_env * 0.65;
            mix_l += s;
            mix_r += s;
            self.player_laser_env *= 0.99945;
        }

        // 2. Enemy Alien Soliton Laser (Warbling Bohmian ring-modulated sweep 760 Hz -> 210 Hz)
        if self.enemy_laser_env > 0.001 {
            self.enemy_laser_freq = (self.enemy_laser_freq * 0.99955).max(210.0);
            self.enemy_laser_phase =
                (self.enemy_laser_phase + tau * self.enemy_laser_freq * dt).rem_euclid(tau);
            self.enemy_mod_phase = (self.enemy_mod_phase + tau * 34.0 * dt).rem_euclid(tau);
            let sig = self.enemy_laser_phase.sin()
                * (0.65 + 0.35 * self.enemy_mod_phase.cos())
                * self.enemy_laser_env
                * 0.60;
            let gl = (0.5 * (1.0 - self.enemy_pan)).sqrt();
            let gr = (0.5 * (1.0 + self.enemy_pan)).sqrt();
            mix_l += sig * gl;
            mix_r += sig * gr;
            self.enemy_laser_env *= 0.99955;
        }

        // 3. Crystalline Crack / Impact Strike (Mineral-tuned metallic/quartz chime)
        if self.crystal_env > 0.001 {
            self.crystal_phase1 =
                (self.crystal_phase1 + tau * self.crystal_freq * dt).rem_euclid(tau);
            self.crystal_phase2 =
                (self.crystal_phase2 + tau * (self.crystal_freq * 1.52) * dt).rem_euclid(tau);
            let sig = (0.65 * self.crystal_phase1.sin() + 0.35 * self.crystal_phase2.sin())
                * self.crystal_env
                * 0.72;
            let gl = (0.5 * (1.0 - self.crystal_pan)).sqrt();
            let gr = (0.5 * (1.0 + self.crystal_pan)).sqrt();
            mix_l += sig * gl;
            mix_r += sig * gr;
            self.crystal_env *= 0.99950;
        }

        // 4. Asteroid Topological Cleave / Cavitation Shatter Explosion!
        // Combines a deep shockwave punch (148 Hz -> 36 Hz) + crystalline shard partials + broadband shatter roar.
        if self.break_env > 0.001 {
            self.break_sub_freq = (self.break_sub_freq * 0.99982).max(36.0);
            self.break_sub_phase =
                (self.break_sub_phase + tau * self.break_sub_freq * dt).rem_euclid(tau);
            self.break_shard_phase =
                (self.break_shard_phase + tau * 580.0 * dt).rem_euclid(tau);

            self.noise_state ^= self.noise_state << 13;
            self.noise_state ^= self.noise_state >> 17;
            self.noise_state ^= self.noise_state << 5;
            let white = (self.noise_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
            self.lp_noise_l += 0.18 * (white - self.lp_noise_l);
            self.lp_noise_r += 0.18 * (-white - self.lp_noise_r);

            let sub_punch = (self.break_sub_phase.sin()
                + 0.5 * (2.0 * self.break_sub_phase).sin())
                * 0.85;
            let shard_ring = (self.break_shard_phase.sin()
                + 0.4 * (1.618 * self.break_shard_phase).sin())
                * 0.35;

            let gl = (0.5 * (1.0 - self.break_pan * 0.7)).sqrt();
            let gr = (0.5 * (1.0 + self.break_pan * 0.7)).sqrt();

            mix_l += (sub_punch + shard_ring + self.lp_noise_l * 0.95) * self.break_env * gl * 0.92;
            mix_r += (sub_punch + shard_ring + self.lp_noise_r * 0.95) * self.break_env * gr * 0.92;
            self.break_env *= 0.99984; // ~320 ms visceral explosion tail
        }

        // 5. Antiphase Shield Parry / Bow-Shock Sweep (240 Hz -> 760 Hz upward harmonic sweep)
        if self.shield_env > 0.001 {
            self.shield_freq = (self.shield_freq * 1.00045).min(760.0);
            self.shield_phase =
                (self.shield_phase + tau * self.shield_freq * dt).rem_euclid(tau);
            let sig = (self.shield_phase.sin() + 0.5 * (1.5 * self.shield_phase).sin())
                * self.shield_env
                * 0.55;
            mix_l += sig;
            mix_r += sig;
            self.shield_env *= 0.99950;
        }

        ((mix_l * 0.85).tanh(), (mix_r * 0.85).tanh())
    }
}

impl Iterator for FieldModalSynthesizer {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<f32> {
        if let Some(r) = self.pending_right.take() {
            Some(r)
        } else {
            let (l, r) = self.next_stereo_frame();
            self.pending_right = Some(r);
            Some(l)
        }
    }
}
