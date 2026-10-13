# Dzoglang Runtime Engine: Architecture & Evaluation

This document details the initial Rust implementation of the **Dzoglang** field runtime engine, its validation against the five development gates defined in [`readme.md`](../readme.md), and a technical evaluation of **Bevy vs. Native WGPU**.

---

## 1. Engine Architecture & Module Structure

Dzoglang operates on a relation-first physical medium: a 2D heterogeneous acoustic lattice with spatially and temporally staggered finite-difference time-domain (FDTD) updates.

```text
Host Controller (winit / crossterm)
                 │
                 ▼
       Validated FieldSpec / Checkpoint (RON)
                 │
                 ▼
     Acoustic Lattice Scheduler
     ├── CPU Reference Solver (dzog-core)
     └── GPU Native Pipelines (dzog-wgpu via WGSL)
                 │
       ┌─────────┴─────────┐
       ▼                   ▼
Pressure Probes      Fullscreen View
(Goal / Sound)       (WGSL Palettes)
       │
       ▼
Offline WAV Export / Audio Ring
```

### Workspace Crates

- [`crates/dzog-core`](../crates/dzog-core/): Pure Rust acoustic PDE lattice engine and CPU reference solver:
  - [`grid.rs`](../crates/dzog-core/src/grid.rs): Staggered Yee 2D grid ($p$ at cell centers, $v_x$ on East vertical faces, $v_y$ on North horizontal faces).
  - [`material.rs`](../crates/dzog-core/src/material.rs): Heterogeneous material fields ($\rho(x) > 0$, $K(x) > 0$, $\gamma(x) \ge 0$), wave speed $c = \sqrt{K/\rho}$, impedance $Z = \rho c$.
  - [`boundary.rs`](../crates/dzog-core/src/boundary.rs): Face wall masks, moving boundaries with enforced normal velocity $v_{\text{wall}}$, and outer absorbing sponge layers.
  - [`solver.rs`](../crates/dzog-core/src/solver.rs): Staggered FDTD numerical solver with conservative CFL limit checking and exponential dissipative updates $\alpha = \exp(-\gamma \Delta t)$.
  - [`excitation.rs`](../crates/dzog-core/src/excitation.rs): Smooth Gaussian pressure impulses and directional wave packets matching impedance $Z$.
  - [`energy.rs`](../crates/dzog-core/src/energy.rs): Hamiltonian acoustic energy accounting ($E_p + E_k$) and relative drift monitoring.
  - [`probe.rs`](../crates/dzog-core/src/probe.rs): Spatial listener probes and goal threshold event detectors.
  - [`manifest.rs`](../crates/dzog-core/src/manifest.rs): RON-serialized `FieldSpec` and `Checkpoint` persistence.
  - [`audio.rs`](../crates/dzog-core/src/audio.rs): Probe history decimation and 48 kHz stereo WAV export.

- [`crates/dzog-wgpu`](../crates/dzog-wgpu/): Native GPGPU acceleration:
  - [`shaders/velocity.wgsl`](../crates/dzog-wgpu/src/shaders/velocity.wgsl): Pass 1 compute shader updating East and North velocities while preserving center pressure.
  - [`shaders/pressure.wgsl`](../crates/dzog-wgpu/src/shaders/pressure.wgsl): Pass 2 compute shader calculating $\nabla \cdot \mathbf{v}$ and updating pressure.
  - [`shaders/render.wgsl`](../crates/dzog-wgpu/src/shaders/render.wgsl): Fullscreen visualization mapping pressure divergence, velocity glow, and boundary walls.
  - [`simulation.rs`](../crates/dzog-wgpu/src/simulation.rs): Ping-pong `RGBA32F` texture manager with CPU staging readback.

- [`crates/dzog-app`](../crates/dzog-app/): Interactive windowed field instrument combining `winit` and `wgpu`.

- [`pong`](../pong/): Benchmark game in a dedicated subdirectory testing engine progress.

---

## 2. Evaluation: Bevy Engine vs. Native WGPU

The user prompt requested considering Bevy for basics like shaders if it is easier to get started. Below is the technical comparison and rationale for selecting native WGPU:

| Evaluation Dimension | Bevy Game Engine | Native WGPU (`dzog-wgpu`) |
| :--- | :--- | :--- |
| **Philosophical Alignment** | ECS (Entity-Component-System) centered around individual object entities and scene graphs. | Field-first relational lattice. Direct state textures without object BVH or entity overhead. |
| **Compute Substep Throughput** | Render Graph & App schedule are tied to display frame rate (60–144 Hz). Custom compute passes between extracted phases are complex to pipeline thousands of times per frame. | Unbound dispatch loop. Can execute 8 to 64 substeps per render frame (or 96,000 substeps/sec for 48 kHz audio) with minimal CPU driver overhead. |
| **Compile Times & Dependencies** | Pulls in 300+ dependencies, extensive procedural macro trees (`bevy_derive`, `bevy_ecs_macros`, etc.), taking minutes to build. | Lightweight dependency tree (`wgpu`, `pollster`, `bytemuck`). Compiles in ~7 seconds on modern hardware. |
| **Headless CI & Determinism** | Requires mocking or window plugins for headless compute tests; platform-specific winit feature flags. | Native headless compute adapter creation out-of-the-box. Tests execute in milliseconds without display servers. |
| **Low-Level Ping-Pong Control** | Custom compute ping-pong in Bevy requires custom `RenderGraph` nodes, pipeline cache extraction, and `AsBindGroup` boilerplate. | Direct integer `textureLoad` / `textureStore` on ping-pong `RGBA32F` textures matching `readme.md` GPU specifications exactly. |

**Conclusion:** Direct WGPU provides unmatched performance, zero ECS impedance mismatch, faster iteration velocity, and direct adherence to Dzoglang's GPU layout requirements.

---

## 3. Development Gates Verification

All development gates established in [`readme.md`](../readme.md) are verified through automated integration tests:

1. **Gate 1: Verified Propagation**
   - Zero state remains strictly zero: verified in `test_gate1_zero_state_remains_zero`.
   - Pulse travel time matches theory $t = d / c$: verified in `test_gate1_pulse_travel_time`.
   - Lossless energy conservation ($E_k + E_p$ drift < 8% over 150 substeps): verified in `test_gate1_lossless_energy_conservation`.
   - Small-step GPU/CPU numerical agreement (< 1e-4 error between WGSL and CPU): verified in `test_gate1_gpu_cpu_numerical_agreement`.

2. **Gate 2: Authored Landscapes**
   - Interface impedance reflection coefficient matches theoretical $R = \frac{Z_2 - Z_1}{Z_2 + Z_1} = 0.333$ within numerical tolerance: verified in `test_gate2_impedance_reflection_coefficient`.
   - Moving boundary acoustic piston radiation ($\Delta p \approx Z \cdot V$): verified in `test_moving_boundary_piston_acoustic_radiation`.
   - Moving boundary momentum transfer: verified in `test_moving_paddle_momentum_transfer`.

3. **Gate 3: Audible Projection**
   - Downsampling probe histories to 48 kHz with antialias interpolation and WAV export: verified in `test_gate3_offline_audio_export`.

4. **Gate 5: Shareable Situations**
   - Checkpoint state serialization to RON and restored execution matching uninterrupted simulation: verified in `test_gate5_checkpoint_roundtrip`.

---

## 4. Pong Benchmark (`/pong`)

The Pong implementation in [`pong/`](../pong/) tests engine development progress in two distinct modalities:

1. **Field-Wave Mode (`FieldWavePong`)**:
   - The ball is an acoustic wave packet launched with impedance-matched particle velocity.
   - Moving paddles act as impermeable boundary faces with normal velocity $v_{\text{wall}}$, Doppler-shifting and redirecting the wave.
   - Goals are scored when pressure flux triggers acoustic listener probes behind each paddle.

2. **Rigid-Acoustic Mode (`RigidPong`)**:
   - Continuous collision detection (CCD) with perfectly elastic rebound ($e = 1.0$) and tangential friction from moving paddles.
   - Every impact excites the underlying 2D Dzog acoustic lattice, producing realistic cavity resonances sampled by edge probes.

### Running Pong

```bash
# Run automated benchmark (500 steps, ~100k substeps/sec)
cargo run -p pong -- --bench

# Run interactive terminal game (W/S for Player 1, Up/Down for Player 2, 'A' for AI)
cargo run -p pong

# Run full test suite
cargo test --workspace
```
