# Dzog Audio Overview: Field-Grounded Sonification & `bevy_audio` Pipeline

Dzog generates all runtime audio directly from physical field interactions, material wave speeds, and listener geometry—using zero pre-recorded `.wav` sound effects during gameplay.

---

## 1. Core Design Principles

1. **Silent Quantum Vacuum**:
   - Empty space is a clean vacuum. Moving, rotating, or coasting through empty space stamps reflective hull boundaries with zero piston velocity (`wall_vx = 0.0, wall_vy = 0.0`) and gates sub-threshold ripples (`|p| < 0.15`), producing exact `(0.0, 0.0)` silence rather than "underwater" sloshing.
   - Sound is emitted **only** on energetic field events: soliton weapon fire, crystalline impacts, topological cleaving/cavitation explosions, and antiphase shield parries.
2. **Material-Tuned Modal Physics**:
   - Strike pitches and harmonic partials derive from the struck body's constitutive acoustic wave speed:
     $$c(\mathbf{x}) = \sqrt{\frac{K(\mathbf{x})}{\rho(\mathbf{x})}}, \qquad f_0 = 440 \, c(\mathbf{x})$$
   - High-speed `PiezoQuartz` ($c = 1.48$) rings at high crystalline frequencies; dense `ObsidianAnvil` ($c = 1.01, Z = 2.02$) produces low metallic clangs; `BismuthCrystal` ($c = 1.10$) sits between them.
3. **Host Delegation to Bevy (`AGENTS.md` Principle #4)**:
   - `crates/dzog-core/src/audio.rs` contains only pure, host-agnostic DSP math (`BinauralCockpitSonifier`, `FieldAcousticFrame`, `FieldModalSynthesizer`).
   - Hardware device initialization, thread scheduling, sample-format negotiation, and playback control (`AudioSink`) are delegated to Bevy's native `bevy_audio` (`Decodable` / `AudioPlayer`).

---

## 2. Architecture & Data Flow

```text
PDE Lattice (AcousticState, MaterialField) + Physical Step Events
       |
       |  (60 Hz macro-step: sample_full_field + trigger_*)
       v
Arc<Mutex<FieldAcousticFrame>>   [merge_telemetry preserves pending trig_* flags]
       |
       |  (48,000 Hz audio thread: Bevy Decodable -> DzogFieldDecoder)
       v
FieldModalSynthesizer::next_stereo_frame() -> (f32, f32)
       |
       v
bevy_audio (AudioPlayer + PlaybackSettings::LOOP + Query<&AudioSink>)
```

---

## 3. Core Primitives (`crates/dzog-core/src/audio.rs`)

### A. `BinauralCockpitSonifier`
Samples port and starboard wingtip pressures at $\mathbf{x}_{\text{ship}} \pm b\,\hat{\mathbf{n}}_{\perp}(\theta)$:
- **DC Blocker & Limiter**: 1st-order high-pass filter ($y[n] = x[n] - x[n-1] + 0.992\,y[n-1]$) followed by $\tanh$ soft clipping.
- **Doppler Telemetry**: Computes radial closing speed between the listener and active wave packets to derive the instantaneous Doppler multiplier:
  $$f_D = \text{clamp}\!\left(1.0 + 0.45\,\bar{v}_{\text{closing}} + 0.25\,|\mathbf{v}_{\text{ship}}|,\; 0.65,\; 1.65\right)$$

### B. `FieldAcousticFrame`
Shared state bridging the $60\text{ Hz}$ simulation loop and the $48\text{ kHz}$ audio synthesizer:
- **Toroidal Stereo Panning (`compute_stereo_pan`)**: Projects the shortest toroidal delta vector $\Delta \mathbf{x}$ onto the ship's starboard normal $\hat{\mathbf{n}}_{\text{right}} = (-\sin\theta, \cos\theta)$ to pan events across $[-0.85, +0.85]$.
- **Non-Destructive Telemetry Merge (`merge_telemetry`)**: Updates continuous HUD metrics (`rms_left`, `rms_right`, `doppler_factor`, `bohmian_q_mod`) each simulation step without clearing unconsumed transient `trig_*` flags.

### C. `FieldModalSynthesizer` (5 Physical Voices at $48\text{ kHz}$)
Implements `Iterator<Item = f32>` yielding interleaved stereo samples:

| Voice | Trigger Method | Synthesis Model |
| --- | --- | --- |
| **1. Player Soliton Laser** | `trigger_player_laser(overcharged)` | Downward FM phase sweep ($1180 \to 165\text{ Hz}$ normal; $1480 \to 165\text{ Hz}$ overcharged) over $\sim 95\text{ ms}$. |
| **2. Alien Soliton Laser** | `trigger_enemy_laser(pan)` | Stereo-panned downward sweep ($760 \to 210\text{ Hz}$) with $34\text{ Hz}$ Bohmian ring modulation. |
| **3. Crystal Crack / Strike** | `trigger_crystal_crack(pan, c)` | Dual-partial percussive chime at $f_0 = 440\,c$ and $1.52\,f_0$ tuned to mineral wave speed $c = \sqrt{K/\rho}$. |
| **4. Cavitation Break** | `trigger_cavitation_break(pan, intensity)` | Sub-bass shock punch ($148 \to 36\text{ Hz}$) + inharmonic shard partials ($580\text{ Hz}, 938\text{ Hz}$) + 1-pole low-passed Xorshift32 broadband shatter roar ($\sim 320\text{ ms}$ tail). |
| **5. Antiphase Shield / Brake** | `trigger_shield_pulse(parried)` | Upward harmonic phase-cancellation sweep ($240 \to 760\text{ Hz}$). |

When the sum of active voice envelopes falls below `0.0015`, the synthesizer short-circuits to exact `(0.0, 0.0)` silence.

### D. Offline Probe Export (`Gate 3`)
For deterministic offline analysis and test verification (`crates/dzog-core/tests/gate_tests.rs`):
- `resample_probe_to_audio(&PressureProbe, &AudioExportConfig) -> Vec<f32>`: Linearly interpolates step-indexed probe histories to $48\text{ kHz}$ with $\tanh$ limiting.
- `write_stereo_wav(path, sample_rate, left, right)`: Exports 16-bit PCM stereo `.wav` files via `hound`.

---

## 4. Bevy Integration (`asteroids/src/gui.rs`, `pong/src/gui.rs`)

Both GUI crates wrap `FieldModalSynthesizer` in a Bevy `Decodable` asset:

```rust
#[derive(Asset, TypePath, Clone)]
pub struct DzogFieldAudioSource {
    pub shared_frame: Arc<Mutex<FieldAcousticFrame>>,
}

impl Decodable for DzogFieldAudioSource {
    type DecoderItem = f32;
    type Decoder = DzogFieldDecoder; // wraps FieldModalSynthesizer (2 ch, 48,000 Hz)

    fn decoder(&self) -> Self::Decoder {
        DzogFieldDecoder(FieldModalSynthesizer::new(self.shared_frame.clone()))
    }
}
```

- **Registration**: `.add_audio_source::<DzogFieldAudioSource>()` on `App`.
- **Playback**: Spawned once in `setup_scene` as `(AudioPlayer(source_handle), PlaybackSettings::LOOP)`.
- **Controls**:
  - Press **`[N]`** in-game to toggle mute/unmute via Bevy's `Query<&AudioSink>` (`sink.toggle()`).
  - Pass `--mute` on the CLI (`cargo run -p asteroids -- --mute`) to skip spawning the audio player.
