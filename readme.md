# Dzoglang

A relation-first programming and artistic medium. Author a landscape of laws,
materials, boundaries, and excitations; encounter its behavior through image,
sound, and direct intervention.

Status: active Rust/WGPU runtime and interactive field demos implemented (`dzog-core`, `dzog-wgpu`, `dzog-app`, `pong`).

Read [philosophy.md](./philosophy.md) for the principles and limits behind this
medium, and [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) for the field-first
architecture.

## Current State & Workspace Crates

| Crate | Role | Status |
| --- | --- | --- |
| `crates/dzog-core` | 2D Yee-staggered FDTD acoustic/soliton solver, heterogeneous materials ($\rho, K, \gamma$), continuous cohesion & overpressure erosion, Bohm-Hiley quantum potential ($Q = -\nabla^2 R / R$), moving boundaries, Hamiltonian energy accounting, pressure probes, offline WAV export, RON `FieldSpec` & `Checkpoint` serialization. | Implemented & tested |
| `crates/dzog-wgpu` | GPU-native compute pipeline (`velocity.wgsl`, `pressure.wgsl`, `render.wgsl`) with ping-pong `RGBA32Float` state textures, material/boundary textures, and verified CPU/GPU numerical agreement ($< 10^{-4}$). | Implemented & tested |
| `crates/dzog-app` | Native `winit` + `wgpu` interactive acoustic field canvas for direct wave excitation and boundary inspection. | Implemented |
| `pong` | **Field-First Soliton Breakout** (Bevy 0.15 GUI + headless benchmark): demonstrates point-of-impact directional fracturing, semi-permeable cracked bricks, acoustic wave-induced cleaving (domino chain reactions), heterogeneous brick media, Bohmian + radiation pressure ball guidance, counterfactual branching, and multi-view inspection. | Implemented & tested |

## Running the Demos & Tests

### 1. Field-First Soliton Breakout (`pong`)

Launch the interactive Bevy 0.15 retro CRT GUI:

```bash
cargo run -p pong
```

**Controls & Interactive Capabilities:**
- `A` / `D` or `Left` / `Right`: Move player paddle (imparts moving boundary velocity $v_n$ to the Yee lattice).
- `Space`: Launch ball from paddle (or restart after Game Over / Victory).
- `T`: Toggle **Sonic Blast & Field Coupling (`ON` / `OFF`)** — counterfactual comparator between full field-coupled physics (cavitation shockwaves, $-\nabla p$ radiation force, $-\nabla Q$ Bohmian guidance, wave-induced brick cleaving) and decoupled classical kinematics.
- `V`: Cycle **Perceptual Field Views** (`philosophy.md` §Multiple Views):
  1. *Arcade + Wavefield*: Compressions (amber), rarefactions (cyan), velocity shimmer, and procedural fracture veins.
  2. *Pressure $p$ & Bohmian $Q$*: Scalar pressure and Bohm-Hiley quantum potential guidance ridges.
  3. *Velocity $\mathbf{v}$ & Energy Flux*: Particle velocity magnitude and acoustic Poynting flux ($p\mathbf{v}$).
  4. *Impedance $Z$ & Cohesion $C$*: Material impedance ($\sqrt{\rho K}$) and structural cell cohesion ($C(x,y) \in [0, 1]$).
- `B` / `U`: **Save (`B`) / Restore (`U`) Branch Checkpoint** — snapshot the full field and game state, toggle `T` or paint a barrier, and compare how the situation unfolds.
- `Left-Click`: Inject a live Gaussian acoustic pressure impulse at the cursor (deflect the ball or cleave weakened bricks with sound).
- `Right-Click` (hold/drag): Paint high-impedance acoustic reflector barriers into the arena.
- `P`: Pause / resume simulation.
- `R`: Reset arena.

Run the headless physics & rasterization benchmark:

```bash
cargo run -p pong -- --bench
```

### 2. Native WGPU Acoustic Field Instrument (`dzog-app`)

Launch the standalone `winit` + `wgpu` GPU compute field canvas:

```bash
cargo run -p dzog-app
```

- `Left-Click` / `Drag` or `Space`: Inject Gaussian pressure impulses into the GPU lattice.
- `P`: Pause / resume GPU compute stepping.
- `C`: Clear field state to zero.
- `Esc` / `Q`: Exit.

### 3. Automated Validation Suite

Run all 21 unit, gate, GPU/CPU agreement, and field-physics validation tests across the workspace:

```bash
cargo test --workspace
```

## First Experiment

Build a small two-dimensional acoustic canvas. Paint reflective boundaries,
material impedance, and damping; inject pressure disturbances; see propagation
and interference; listen to pressure sampled at two probes.

The visual and audio views share evolving field state. No collision event selects
a prerecorded sound. Initially this is a wave instrument, not a solid-body engine,
quantum simulator, or general-purpose language.

The authoring cycle remains:

```text
Encounter -> Perturb -> Compare -> Commit -> Compose
```

## Example Session

1. Excite a homogeneous field with a bounded, spatially smooth pressure impulse.
2. Paint fixed walls to form a cavity; compare its reflections with the original.
3. Change density and bulk modulus in a region. Inspect its wave speed and
   impedance, then compare the reflected and transmitted pressure.
4. Paint a damping gradient and measure how much reflection remains.
5. Position two listener probes. Render a short audio clip from their pressure
   histories, then attempt live playback after the performance gate passes.
6. Branch the situation, alter one boundary, and repeat the same excitation.
7. Share the field specification or checkpoint. A recipient predicts and changes
   its response without reconstructing an object model.

Painting changes an explicit field channel or boundary. Injecting energy changes
dynamic state. Changing the palette changes only presentation.

## Scope

| Capability | First implementation |
| --- | --- |
| Substrate | Small regular 2D grid; begin at 64 x 64 and benchmark larger grids. |
| Dynamic state | Acoustic pressure and two components of particle velocity. |
| Material fields | Positive density and bulk modulus; nonnegative damping. |
| Boundaries | Fixed impermeable faces; optional outer damping sponge. |
| Input | Bounded pressure sources and paused material/boundary brushes. |
| Output | Pressure/velocity views, stereo pressure probes, numerical inspector. |
| Time | Fixed numerical substeps, pause, reset, step, recorded interventions. |
| Persistence | Field specification, complete checkpoint, optional event history. |
| Sound | Offline pressure-to-audio first; live asynchronous playback second. |

Initial materials remain stationary. Acoustic particle velocity is not the
transport velocity of a movable solid. Editing materials during playback and
their associated energy accounting require a later, explicit policy.

## Field Laws

Start with linear acoustics in a heterogeneous stationary medium, not a combined
Gross-Pitaevskii, elasticity, Maxwell, and optical model.

```text
rho(x) * dv/dt = -grad(p) - rho(x) * gamma(x) * v
dp/dt = -K(x) * div(v) - gamma(x) * p + s(x, t)

c(x) = sqrt(K(x) / rho(x))
Z(x) = rho(x) * c(x)
```

`p` is pressure perturbation, `v` acoustic particle velocity, `rho` mass density,
`K` bulk modulus, `gamma` loss rate, and `s` a pressure-rate source. Begin with
declared normalized units. Physical SI calibration is a separate task.

Variable impedance produces partial reflection and transmission. It is not an
absolute barrier. An impermeable face imposes zero normal velocity and has
separate, explicit semantics. Sponge damping reduces outgoing waves but is not
a perfectly matched layer or an exact nonreflecting boundary.

This model supports interference and cavity resonance. It does not support
shear waves, deforming bodies, collision contact, solitons, or electromagnetic
forces. Its visible field is not optical light propagation.

## Discretization

Use a spatially staggered finite-difference grid: pressure at cell centers,
horizontal velocity on vertical faces, and vertical velocity on horizontal faces.
Use time-staggered updates, with pressure at integer steps and velocity at half
steps. Define outer faces and edge rules explicitly; never rely on out-of-range
texture loads as boundary conditions.

At a material interface, derive face density from adjacent cell densities. Begin
with their arithmetic mean, so inverse face density is `2 / (rho_left + rho_right)`.
Verify impedance behavior and convergence against simple reference cases.

Implement losses with a separately verified dissipative update using nonnegative
exponential decay factors, rather than an unchecked explicit damping decrement.
Document its ordering relative to the conservative update. Initialize half-step
velocities from declared initial conditions using the same convention as the
reference solver; a checkpoint stores already-staggered state.

Choose a conservative CFL bound for the lossless update:

```text
c_bound = sqrt(K_max / rho_min)
dt_cfl = safety / (c_bound * sqrt(1/dx^2 + 1/dy^2))
0 < safety < 1
dt <= dt_cfl
```

This uses supported material bounds rather than assuming local cell wave speeds
alone bound every heterogeneous stencil. Validate the implemented operator and
boundary rules; a formula is not a substitute for stability tests.

Choose `dt` before running and keep it fixed. Reject edits outside the declared
material envelope or resource budget rather than silently changing step size.
Retiming a staggered solver requires explicit state conversion.

Resolve wavelengths with multiple cells and establish a useful frequency band
through convergence tests. CFL stability does not prevent numerical dispersion,
under-resolved features, or inaccurate reflection. Limit source amplitude and
bandwidth to the tested regime; the initial model is linear.

## GPU Layout

Begin with two ping-pong `RGBA32F` state textures, one material texture, and an
explicit face-wall mask. State texels pack quantities at different staggered
locations; sharing a texel does not make them collocated.

| Resource | Logical contents |
| --- | --- |
| State A / B | Center pressure, east-face velocity, north-face velocity, reserved. |
| Material | Center density, bulk modulus, damping, reserved. |
| Wall mask | Blocked faces with documented orientation and outer-edge rules. |
| Probe output | Timestamped pressure samples in a GPU storage buffer. |
| Readback slots | Rotating staging buffers, never reused while mapped/in flight. |

For each numerical substep:

1. Update velocity from State A into State B, preserving pressure.
2. Update pressure from State B into State A, preserving the new velocity.
3. Sample probes when required and record their simulation indices.

Apply damping, sources, and wall conditions at defined points in these passes.
Each pass reads an immutable input and writes a distinct output. A workgroup
barrier cannot synchronize the entire lattice; neighbor-dependent stages use
separate dispatches with valid WGPU resource usage transitions.

Use integer `textureLoad` for stencils, not interpolated sampling. Check adapter
format capabilities and limits; do not assume filterable 32-bit float textures
or simultaneous storage reads/writes are supported.

A fullscreen render pass reads completed state and maps selected quantities to
color. Rendering does not advance the simulation.

## Audio And Time

Audio-rate simulation is a principal engineering risk, not a final hookup step.
A renderer running at 60-240 Hz cannot produce a 48 kHz pressure history by
reading one value per frame. Interpolation cannot recover missing frequencies.

Declare `a`, the amount of simulation time per second of playback. A simulation
frequency `f` is heard at `a * f`; this mapping is explicit sonification unless
the units and time scale have been physically calibrated.

For a source sample rate `Fs`, initially 48 kHz:

```text
dt_sample = a / Fs
m = max(1, ceil(dt_sample / dt_cfl))
dt = dt_sample / m
```

Hold this configuration fixed for a run. Advance `m` substeps per source audio
frame. Band-limit excitation and low-pass probe histories before decimation
when substeps contain frequencies above the output Nyquist limit.

Even `m = 1` implies about 96,000 lattice dispatches per playback second for the
two-pass solver, before probe work. Small GPU dispatch overhead may dominate.
Batch substeps and samples into command submissions, but do not assume batching
eliminates their cost. Profile offline generation before promising live audio.

Each listener samples a normalized, local weighted average of pressure. A whole
boundary pressure sum can cancel useful modes and is neither a microphone nor
pressure flux; physical energy flux involves pressure times velocity.

An impulse can excite cavity modes, but the uniform linear model does not
spontaneously generate new frequencies or material-specific metallic timbre.
Its resonance is geometry and impedance made audible, not a collision synthesizer.

Proposed live path:

```text
GPU probe history -> asynchronous staging readback
                  -> ordered CPU sample blocks
                  -> filtering / resampling / gain / output protection
                  -> bounded SPSC audio ring -> cpal callback
```

The producer follows audio consumption with bounded lookahead, not display
frames. Negotiate device sample rate and format; resample the source stream if
needed without changing the numerical solver step. Start muted with capped gain,
DC removal, and output limiting clearly identified as playback processing.

The callback only consumes prepared frames and writes the device format. It must
not dispatch GPU work, wait for mapping, lock a shared mutex, allocate, or log.
On underrun, write silence and update an atomic counter. Preserve block order
and indices even when readback callbacks complete out of order.

There is no directly shared live GPU/audio-driver ring in the portable WGPU path:
mapped buffers belong to the CPU and cannot simultaneously be used by the GPU.
Poll completion off the audio thread, copy samples, unmap, then recycle a slot.

Measure buffering and device latency. Tag display snapshots with simulation
time; compare them with the estimated playback position. Define pause, reset,
and edit behavior by draining or invalidating queued audio and re-priming it.
Underruns, suspension, or device loss must report discontinuity, not silently
claim synchronization. Offline sound remains useful if live deadlines cannot
be met; a separate CPU or temporally tiled solver would be an explicit redesign.

## Image As Program

The canonical source is a field specification: typed spatial layers plus a small
manifest declaring their interpretation. Images are authorable data, not magic
instructions inferred from color.

The manifest includes law/solver identity, grid dimensions and spacing, units,
time scale, material bounds, boundary convention, initial conditions, sources,
probes, and presentation mappings. It contains no user control-flow script in
the first prototype; the host still performs validation, stepping, and I/O.

Start with a directory bundle containing a RON manifest and explicitly shaped,
little-endian float32 field payloads plus a face mask. PNG can serve as a preview
or a documented, quantized material import. Ordinary PNG cannot hold arbitrary
multichannel floating-point state. Multichannel float32 OpenEXR is a possible
later interchange format; channel metadata and laws remain necessary.

Distinguish three artifacts:

- **Specification:** authored materials, boundaries, laws, inputs, and initial state.
- **Checkpoint:** all dynamic fields, current material and wall layers, step/time
  indices, temporal staggering, source state, and solver configuration at one
  step boundary.
- **Performance:** initial specification/checkpoint plus ordered, step-indexed edits
  and excitations; include audio mapping when reproducing sonification.

Copy checkpoint resources at one complete-step boundary so all channels describe
the same moment. Strip GPU row padding when serializing; validate dimensions,
payload lengths, finite values, coefficient ranges, and allocation limits before
upload. A screenshot or pressure texture alone is not a resumable simulation.

Expect tested numerical agreement on the same solver/backend, not bit-identical
cross-GPU results. A checkpoint restores the simulation, not already queued
device sound; re-prime playback. Exact audio exports require preserving the
rendered samples or every relevant resampler/filter state.

## Host Architecture

```text
winit input + inspector
          |
          v
validated field specification / step-indexed interventions
          |
          v
Rust scheduler -> wgpu compute substeps -> completed field state
                                      |                |
                                      v                v
                               pressure probes     render view
                                      |
                                      v
                             readback -> audio ring -> cpal
```

Use Rust with `winit`, `wgpu`, and `cpal`; `serde`/RON for the manifest and a
bounded SPSC ring implementation for audio. Add a small WGPU-compatible inspector
only as needed. The host manages clocks, validation, persistence, and observation;
it does not attach gameplay scripts to every visible pattern.

Keep a tiny CPU reference solver with matching discretization for numerical
tests, not as a second interactive engine. Start with one crate, a few Rust
modules, and velocity, pressure, probe, and render shaders. Avoid a plugin system
or generalized physics abstraction until a second validated model needs one.

Provide keyboard brush placement, exact parameter entry, pause/step inspection,
muted operation, and reduced-motion display. Sound and color cannot be the only
ways to understand an authored commitment.

## Development Gates

### 1. Verified Propagation

Implement the CPU reference and GPU solver in a homogeneous field. Test zero
state, pulse travel time, small-step CPU/GPU agreement, bounded lossless energy
error, and grid/time refinement. Reject nonfinite parameters and unstable steps.

### 2. Authored Landscapes

Add paused brushes, impedance changes, impermeable walls, and sponge damping.
Test interface reflection/transmission, cavity modes, zero normal wall velocity,
dissipative loss, and residual sponge reflection. Compare against analytical
cases within a declared numerical tolerance and frequency band.

### 3. Audible Projection

Produce offline probe histories and audio exports before enabling `cpal`.
Verify sample count, pitch/time mapping, antialias filtering, gain safety, and
identical probe histories under different render rates.

### 4. Live Budget

Measure solver throughput, dispatch and readback costs, staging latency, ring
occupancy, underruns, and visual/audio time offsets at stated grid sizes and
devices. Test delayed readback, window suspension, reset, and device failure.
Enable live playback only for workloads that sustain deadlines with headroom.

### 5. Shareable Situations

Round-trip specifications and checkpoints, compare uninterrupted and restored
evolution, and replay the same interventions. Branch a cavity experiment and
paste a field fragment with explicit edge and scale choices.

## Evaluation

Ask one person to author a cavity, damping region, or wave channel and another
to predict and modify its response. Compare with an equivalent parameter-based
simulation interface or node editor, and with a textual description where useful.

Measure prediction accuracy, unintended semantic edits, explanation required,
and successful reuse. A field painter becomes a programming medium only if
people can express, communicate, and compose behavior through it.

If they merely enjoy the display or sound, we have an instrument or artwork.
If numerical complexity obscures relationships, simplify the medium before
adding more physics.

## Claims And Limits

| Proposal | Engineering boundary |
| --- | --- |
| GPU-native lattice | Regular stencils suit GPUs; utilization and speed require measurement. |
| No object BVH | The wave prototype needs none; this does not solve rigid-body contact. |
| Constant work per cell | Total work is O(cells * substeps), including empty regions. |
| Continuous interaction | Finite differences remain discrete, dispersive, and resolution-limited. |
| Shared image/sound origin | Different clocks, readback, filtering, and device buffers add latency. |
| Image persistence | Complete state and metadata are required, not just visible pixels. |

No guarantee of branch-free execution, zero latency, no tunneling, or automatic
near-full GPU saturation follows from using PDEs. High stiffness and finer grids
can increase numerical cost rather than remove it.

## Later Models

Deformable matter needs strain/deformation state, shear behavior, constitutive
laws, and validated contact coupling. Density and bulk modulus alone do not
produce steel or rubber timbre. Magnetism needs a consistent electromagnetic
model and charge/current coupling. Optical reflection requires an explicit
optical model, not merely a displayed refractive-index gradient.

A complex nonlinear wavefield is a separate experiment. The proposed
Gross-Pitaevskii equation is first-order Schrodinger-type evolution, not the
Klein-Gordon equation. Nonlinear sign, dimension, saturation, and discretization
determine whether packets disperse, persist, or collapse; solitons and agents do
not follow automatically. Adding damping also precludes a blanket unitary claim.

Do not couple these models until each has validated equations, units, energy
exchange, and numerical tests. Pong would additionally require controllable
moving boundaries and explicit scoring semantics; a reflecting packet alone
does not supply a game.

## References

- [WGPU buffer mapping](https://docs.rs/wgpu/latest/wgpu/struct.Buffer.html):
  asynchronous mapping and CPU/GPU ownership.
- [CPAL](https://docs.rs/cpal/latest/cpal/): device configuration and audio callbacks.
- [Philosophy](./philosophy.md): conceptual sources and the testable hypothesis.
