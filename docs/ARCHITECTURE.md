# Dzog Runtime Engine: Field-First Architecture & Bevy GUI

This document details the field-first architecture of the **Dzog** runtime engine, adherence to its philosophical principles ([`philosophy.md`](../philosophy.md)), the **Nonlinear Solitary Wave Mechanics** replacing classical rigid bodies, and the **Bevy 0.15 GUI & Pixel Presentation** in [`/pong`](../pong/).

---

## 1. Field-First Philosophy vs. Classical Mechanics

Lines 160–163 of [`philosophy.md`](../philosophy.md) declare:

> *"Objects may later be recognized as persistent patterns of activity. The first linear wave model does not produce autonomous agents, solid bodies, or stable solitons. Emergence is a property to demonstrate, not an implementation shortcut."*

Introducing classical rigid-body mechanics (`Body2D`, bounding boxes, continuous collision sweep queries) would subvert Dzog by treating objects as primary entities and demoting the field to a cosmetic effect.

### The Problem with Pure Linear Waves
In linear acoustics ($\partial^2 p / \partial t^2 = c^2 \nabla^2 p$), wave packets undergo radial cylindrical dispersion ($1/\sqrt{r}$ decay). Without a focusing mechanism, a linear pulse spreads into diffuse background ripples rather than maintaining a trackable, coherent form for 2D gameplay.

### The Field-First Solution: Nonlinear Solitary Field
Instead of adding rigid bodies, we expand the constitutive lattice law with a saturating nonlinear self-focusing term:

$$\rho(x) \frac{\partial \mathbf{v}}{\partial t} = -\nabla p - \rho(x)\gamma(x)\mathbf{v}$$
$$\frac{\partial p}{\partial t} = -K(x)\nabla \cdot \mathbf{v} - \gamma(x)p + f_{\text{nl}}(p) + s(x, t)$$

where:
$$f_{\text{nl}}(p) = \frac{\beta \cdot p}{1 + \alpha \cdot p^2} - \kappa \cdot p$$

- **Self-Cohesion**: Nonlinearity $\beta$ balances numerical dispersion, maintaining a compact localized packet (the **soliton**).
- **Amplitude Saturation**: The denominator $1 + \alpha p^2$ caps growth at $\frac{\beta}{2\sqrt{\alpha}}$, preventing non-physical blowup.
- **Background Filtering**: The sub-threshold decay $\kappa$ extinguishes small dispersive ripples, keeping the field background clean.
- **Pure Emergence**: **Zero rigid-body structs.** The "ball" is recognized purely as a persistent localized pattern of field state ($p, \mathbf{v}$).

---

## 2. Moving Boundary Fields & Pure Field Pong

In [`pong/src/field_game.rs`](../pong/src/field_game.rs):

1. **Paddles as Moving Boundary Fields**:
   Paddles are authored in the lattice face mask (`wall_east[x, y] = true`).
   The player's paddle velocity is enforced as the boundary velocity condition:
   $$v_n = v_{\text{paddle}}$$
   When the soliton packet strikes the paddle, the boundary stencil reflects the wave flux and imparts Doppler momentum without collision queries.

2. **Goal Sensors as Field Probes**:
   Acoustic listener probes placed at the goal net backings sample local pressure flux.
   When the soliton penetrates past a paddle, the probe triggers and records the score.

---

## 3. Pixel Presentation & Bevy 0.15 GUI

Instead of terminal ASCII, the engine renders directly to **pixels and shaders**:

- **Dynamic Texture Pipeline**:
  [`SolitonFieldGame::render_to_pixel_buffer()`](../pong/src/field_game.rs#L180) rasterizes field pressure, kinetic velocity glow, and boundary walls directly into an RGBA8 retro canvas.
- **Retro Aesthetic**:
  Phosphor cyan paddles, golden amber soliton core, electric blue wavefronts, and CRT dark background.
- **Bevy 0.15 Windowing & Mandatory WGPU Compute**:
  [`crates/dzog-bevy/src/lib.rs`](../crates/dzog-bevy/src/lib.rs) and [`crates/dzog-wgpu`](../crates/dzog-wgpu) require a WGPU-capable GPU (`GpuContext::shared()`), executing all FDTD substeps (`velocity.wgsl`, `pressure.wgsl`, `luminescence.wgsl`) and body force reductions (`reduce_body.wgsl`) on the GPU.
- **Asteroids Tech Demo (`asteroids/`)**:
  Runs on a `240×150` periodic/gravity-well lattice in a `1600×1000` window with 3D-faceted mineralogical asteroids (`PiezoQuartz`, `ObsidianAnvil`, `BismuthCrystal`), continuous `SolitonCarrierPulse` laser luminescence, ship-like `BohmianSaucer` corvettes & `HarmonicWeaver` cruisers, player-like autopilot (`asteroids/src/autopilot.rs`, `[T]` / `--autopilot`), and `ffmpeg` MP4 recording (`--record [file.mp4] --duration <secs>`).
- **Live Binaural & Modal Audio (`bevy_audio`)**:
   Cockpits and paddles sample local lattice oscillations into `FieldModalSynthesizer` (`Decodable`).

---

## 4. Verification & Benchmarks

```bash
# Run headless benchmark (1,000 steps, >70,000 substeps/sec on CPU)
cargo run -p pong -- --bench

# Run full test suite across workspace
cargo test --workspace

# Launch Bevy GUI Soliton Pong
cargo run -p pong
```

All 14 workspace tests pass:
- `test_soliton_cohesion_vs_linear_dispersion`: Proves that solitary self-focusing preserves packet peak concentration above dispersive linear decay.
- `test_soliton_wall_reflection_rebound`: Proves soliton reflects elastically off rigid boundaries.
- `test_field_soliton_paddle_reflection`: Validates paddle reflection and rebound in Pong.
- `test_field_soliton_goal_probe_scoring`: Validates probe-based goal detection.
- `test_gate1_gpu_cpu_numerical_agreement`: Verifies WGSL compute matches CPU reference solver within $10^{-4}$.
