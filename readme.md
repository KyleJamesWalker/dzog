# Dzog Simulator
<small>by [Jonathan Dunlap](https://www.linkedin.com/in/jonathandunlap/)</small>

**Note: This is highly experimental work for a new proof of concept simulator using quantum field theory.**

Dzog Simulator is a GPU-accelerated quantum 2D wave-matter field-physics engine in Rust.

Conventional game engines treat space as an empty coordinate system populated by isolated rigid bodies (`struct Asteroid { hp: 2 }`) that ignore one another until a bounding-box check triggers an imperative callback. **Dzog Simulator** replaces object-centric execution with a continuous physical medium governed by wave mechanics, solid-state crystal cohesion, and pilot-wave guidance.

In Dzog Simulator, a scene is an **executable physical situation**: a lattice where every cell carries pressure, velocity, material impedance, and structural cohesion. Ships, destructible asteroids, projectiles, and shields are persistent patterns of matter and wave excitation that radiate wakes, fracture along continuous stress planes, and interfere directly across the medium.

## Interactive Demos

### 1. Crystalline Asteroids & Alien Ecology
> **Live Web Demo:** [**Play in Browser (`jadbox.github.io/dzog/asteroids/`)**](https://jadbox.github.io/dzog/asteroids/)  
> Toroidal & gravity-well wave mechanics, topological BFS crystal cleaving, antiphase shields, and Bohm-Hiley pilot-wave navigation.

https://github.com/user-attachments/assets/63759ad7-6401-47e1-8674-87e3e15a4092

### 2. Crystalline Corridor (Vertical Wave-Stream Shmup)
> **Live Web Demo:** [**Play in Browser (`jadbox.github.io/dzog/shmup/`)**](https://jadbox.github.io/dzog/shmup/)  
> Expands the ecology into a scrolling metamaterial canyon: a situated **Tri-Lobe Crystalline Ship** (where wing lobes physically sever and re-crystallize instead of using integer hitpoints), **Chladni Pilot-Wave Swarms** surfing $-\nabla Q$ standing-wave pockets, and contested **Annealing Power Cubes** ($\Delta C > 0$).

https://github.com/user-attachments/assets/718eeba3-4bc6-4f00-a8dc-97c151ca4618

---

## Quick Start

```bash
# Run the full verification suite (36 analytical, GPU/CPU agreement, and physics tests)
cargo test --workspace

# Launch the Crystalline Corridor Vertical Wave-Stream Shmup (Chladni Swarms & Annealing Cubes)
cargo run -p shmup
cargo run -p shmup -- --autopilot

# Launch the Crystalline Asteroids & Alien Ecology demo (Bevy + WGPU + Live Audio)
cargo run -p asteroids

# Launch Asteroids in Autopilot mode or record an MP4 demo session
cargo run -p asteroids -- --autopilot
cargo run -p asteroids -- --autopilot --record asteroids_demo.mp4 --duration 30

# Launch the Solid-State & Quantum-Vacuum Breakout demo
cargo run -p pong

# Run headless WGPU zero-readback & FDTD benchmarks
cargo run -p shmup -- --bench
cargo run -p asteroids -- --bench
cargo run -p pong -- --bench
```

### Builds and web demos

Each `v*` tag publishes desktop builds (Linux x86_64/aarch64, macOS aarch64, Windows x86_64) and a web build to [Releases](https://github.com/jadbox/dzog/releases); pushes to `main` deploy the web demos to GitHub Pages. To build the web demos locally (requires the `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI matching `Cargo.lock`):

```bash
tools/build_web.sh            # writes dist/web; serve it with any static file server
```

The web builds step the field on the `dzog-core` CPU reference solver, since the browser cannot block on a GPU readback each frame; desktop builds keep the WGPU compute pipeline.

---

## Theoretical Foundation: Field Mechanics as Computation

Traditional engines model worlds as lists of disconnected bounding boxes: empty space carries no tension or memory, interactions are binary collision events, and adding cross-entity behaviors requires wiring brittle $O(N^2)$ state scripts. Dzog Simulator inverts this architecture—**the substrate itself is the computation**. Just as a pixel is a color element and a voxel is a volume element, every cell in a Dzog Simulator lattice is a coupled Hamiltonian field element storing dynamic wave state alongside local material commitments (density, stiffness, damping, and cohesion).

Because the medium mediates all causality, complex gameplay mechanics emerge directly from local physical laws rather than scripted special cases. Drawing on nonlinear wave mechanics, David Bohm and Basil Hiley's *active information* (where the spatial curvature of a wavefield guides trajectories), and James Hillman's concept of the *situated image*, every actor continuously perturbs and senses its neighborhood. Weapons fire self-focusing solitary wave packets (solitons); impacts carve internal fracture waveguides that channel stress until crystals cleave topologically into tumbling fragments; shields cancel incoming fire via antiphase destructive interference; and cockpit probes synthesize real-time Doppler audio directly from local pressure oscillations.

### Core Engine Pillars & Why the Math Matters

1. **Constitutive Material Laws over Object Scripts**:
   Every cell evolves under a unified finite-difference time-domain (FDTD) operator parameterized by density $\rho$, bulk modulus $K$, damping $\gamma$, and cohesion $C \in [0, 1]$.
   *Why it matters*: Wave speed, reflection ratios, recoil momentum, and even the musical pitch of an impact emerge automatically from a body's material properties—no per-object interaction scripts required.
2. **Nonlinear Solitary Cohesion (Solitons)**:
   Standard linear waves disperse radially and fade out. Dzog Simulator adds a saturating nonlinear self-focusing law that counteracts dispersion while scrubbing low-amplitude background noise.
   *Why it matters*: Plasma bolts, phase-pulses, and balls travel and rebound as crisp, self-sustaining wave packets without rigid-body wrappers or artificial velocity clamping.
3. **Bohm-Hiley Active Information & Pilot-Wave Guidance**:
   The solver continuously evaluates the Bohmian quantum potential $Q$, which measures the *normalized spatial curvature (form)* of the wave envelope rather than raw amplitude.
   *Why it matters*: Subtle standing-wave channels and interference ridges exert an organizing guidance force ($-\nabla Q$) on actors, allowing alien craft and Chladni swarm formations (`ChladniCarrierEmitter`) to naturally surf phase valleys and weave around shockwaves without pathfinding graphs or spline scripts.
4. **Continuous Cohesion Erosion, Topological Cleaving & Exciton Annealing**:
   Solid structures are regions of high cohesion $C$ with finite yield thresholds. Overpressure erodes cohesion along directional stress axes, opening semi-permeable fracture waveguides before severing the body. Conversely, absorbing piezoelectric micro-condensates ("Power Cubes") triggers reversible **exciton annealing** ($\Delta C > 0$), regrowing severed structural lobes from the core outward.
   *Why it matters*: Destruction and healing are topological rather than an integer hitpoint counter (`hp: 3`). Striking an existing glowing crack triggers a resonance cleave via connected-component BFS; capturing a field-guided Power Cube physically re-crystallizes lost ship wings.
5. **Counterfactual Situation Branching**:
   Authoring, debugging, and gameplay share a single experimental loop:
   ```text
   Encounter -> Perturb -> Compare -> Commit -> Compose
   ```
   *Why it matters*: Because the entire causal state of the world lives in deterministic lattice buffers, players and authors can snapshot any moment (`[B]`), test a counterfactual action or material change, and restore the branch (`[U]`) with zero state desynchronization.

---

## Numerical Design: Built for Real-Time GPU Execution

Running continuum PDEs at $60\text{–}120\text{ Hz}$ inside an interactive game requires strict numerical discipline so the simulation never explodes into `NaN`s, leaks energy, or stalls the CPU. Full equations, stencils, and GPU architecture are detailed in [**`docs/field_first_whitepaper.md`**](./docs/field_first_whitepaper.md):

- **Yee-Staggered Conservation**: Pressure lives at cell centers and velocity lives on East/North cell faces at half-time steps. This eliminates odd-even checkerboard artifacts and guarantees exact momentum conservation when waves slam into rotating hulls.
- **Exact Fresnel Material Interfaces**: Face densities use adjacent-cell means so high-impedance materials (like `ObsidianAnvil`) naturally reflect glancing shots while piezo crystals (`PiezoQuartz`) transmit fast internal phonons.
- **Blowup-Proof Rational Saturation**: Unlike cubic Kerr nonlinearities that explode when shockwaves collide, Dzog Simulator's rational saturation term mathematically bounds peak self-focusing gain while keeping empty vacuum dead-silent.
- **Unconditionally Positive Exponential Damping**: Losses integrate via $\exp(-\gamma \Delta t)$ rather than Euler subtraction, preventing sign-flip instabilities inside high-absorption sponge boundaries and cracked crystal veins.
- **Hamiltonian Energy Harvesting**: The engine tracks instantaneous potential and kinetic field energy across the lattice—directly powering gameplay when the player's Antiphase Shield cancels an enemy wave-bolt ($\Delta \phi = \pi$) and harvests the annihilated wave energy into the ship's weapon capacitor.

---

## Workspace Architecture

| Crate | Role |
| --- | --- |
| [`crates/dzog-core`](./crates/dzog-core) | Reference Yee-staggered FDTD solver, soliton law, Bohmian potential $Q$, exciton luminescence field $L(\mathbf{x}, t)$, `SolitonCarrierPulse` phase-locking, Poisson gravity $\Phi_g$, rotating `CohesiveBody` 3D facet rasterizer & BFS topological cleaving, thruster plumes, antiphase shields, $48\text{ kHz}$ modal audio synthesizer (`FieldModalSynthesizer`), and RON `FieldSpec` / `Checkpoint` serialization. |
| [`crates/dzog-wgpu`](./crates/dzog-wgpu) | Hardware WGPU compute pipeline (`velocity.wgsl`, `pressure.wgsl`, `luminescence.wgsl`, `impulse.wgsl`, `reduce_body.wgsl`) over ping-pong `RGBA32Float` state & luminescence textures, toroidal periodic wrapping, zero-readback GPU body force/torque/probe reduction (`128` bodies), and verified CPU/GPU agreement ($< 10^{-4}$). |
| [`crates/dzog-bevy`](./crates/dzog-bevy) | Bevy engine plugin (`DzogBevyPlugin`): nearest-neighbor field canvas presentation (`FieldImageHandle`), shared perceptual view modes (`FieldViewMode`), mandatory hardware WGPU compute accelerator (`GpuFieldAccelerator`), and native `bevy_audio` (`Decodable` / `DzogFieldAudioSource`) streaming. |
| [`crates/shmup`](./crates/shmup) | **Crystalline Corridor (Bohm-Hiley Chladni Swarm & Annealing Shmup)**: 4-way planar thruster Tri-Lobe crystal ship (3 structural lobes instead of integer HP), Chladni standing-wave alien swarm formations (`ChladniCarrierEmitter`), contested Annealing Piezo Power-Cubes ($\Delta C > 0$ regrowing severed lobes), and scrolling metamaterial canyon walls. |
| [`asteroids`](./asteroids) | **Field-First Crystalline Asteroids & Alien Ecology** (`240×150` lattice, `1600×1000` window): Multi-tier 3D-faceted mineralogical asteroids (`PiezoQuartz`, `ObsidianAnvil`, `BismuthCrystal`), dynamic field-lit crystal facets ($\nabla L, \nabla p$), continuous soliton laser glow, tactical piezo chain-cleaving, `BohmianSaucer` corvette & `HarmonicWeaver` cruiser craft, autopilot (`[T]` / `--autopilot`), and MP4 session recording (`--record`). |
| [`pong`](./pong) | **Solid-State & Quantum-Vacuum Breakout**: Crystalline phonon coupling, point-of-impact cleavage planes, sympathetic crystal domino cascades, Paddle Doppler Overcharge, coherent Phase-Pulse wave-casting, and counterfactual checkpoint branching. |

### GPU Compute Pipeline (`dzog-wgpu`)

```text
FieldSpec / GPU Impulse Shader (impulse.wgsl) + SolitonCarrierPulse
       |
       v
WGPU Compute Substeps (Pass 1: Velocity + Bohmian Q -> Pass 2: Pressure + Soliton -> Pass 3: Luminescence L)
       |                                                        |
       v                                                        v
Zero-Readback Body Force/Probe Reduction (reduce_body.wgsl)   Bevy Field Canvas & Perceptual Views
       |
       v
48 kHz Modal Synthesizer (FieldModalSynthesizer -> bevy_audio Decodable)
```

| GPU Texture | Format | Packed Channels |
| --- | --- | --- |
| `State A / B` | `RGBA32Float` | `(p_center, vx_east, vy_north, q_bohmian)` |
| `Luminescence A / B` | `RGBA32Float` | `(luminescence_L, 0, 0, 0)` |
| `Material` | `RGBA32Float` | `(density_rho, bulk_modulus_K, damping_gamma, cohesion_C)` |
| `Boundary` | `RGBA32Float` | `(wall_east_mask, wall_vx, wall_north_mask, wall_vy)` |

---

## Engineering Boundaries

| Capability | Engineering Boundary |
| --- | --- |
| **GPU stencil parallelism** | $O(\text{cells} \times \text{substeps})$ uniform dispatch work; active vacuum carries potentials across all cells. |
| **Collision-free wave agency** | Wave interference, radiation pressure, and reflection require zero BVH queries, complemented by compact local-grid body contours. |
| **Discrete continuum limits** | Finite-difference lattices remain bounded by declared CFL time steps and Nyquist spatial wavelengths. |
| **Cross-device reproducibility** | Bit-exact within a backend; CPU and GPU floating-point stencils agree within tested tolerances ($< 10^{-4}$). |

---

## Documentation & References

- [**Philosophy (`philosophy.md`)**](./philosophy.md): Relation-first computing, Bohm-Hiley active information, Hillman's situated image, and living actor ecologies.
- [**Field-First Engine Whitepaper (`docs/field_first_whitepaper.md`)**](./docs/field_first_whitepaper.md): Governing PDEs, saturating soliton math, pilot-wave guidance ($Q$), topological cohesion, and zero-readback WGPU/Bevy architecture.
- [**Audio Overview (`docs/overview_audio.md`)**](./docs/overview_audio.md): Material-tuned modal synthesis, silent quantum vacuum gating, and `bevy_audio` integration.

---

## Asteroids Autopilot & MP4 Video Recording

You can launch `asteroids` with its player-like autopilot enabled (`[T]` in-game or `--autopilot` at startup) and record a 60 FPS MP4 video with synchronized 48 kHz stereo field audio (`<= 1280px` wide, GitHub README CDN ready):

```bash
# Launch with Autopilot enabled (toggle in-game anytime with [T])
cargo run --release -p asteroids -- --autopilot

# Record a 30-second Autopilot session (1200x750 H.264 + AAC stereo field audio) and exit automatically
cargo run --release -p asteroids -- --autopilot --record asteroids_demo.mp4 --duration 30
```

---

## License

Permissive **MIT Attribution License** — see [`LICENSE`](./LICENSE). Forks and redistributions must preserve original author credit (`J. Dunlap / jadbox`).
