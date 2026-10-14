# Field-First Computing: A Wave-Matter & Pilot-Wave Runtime Architecture

**Abstract** — Conventional simulation and game engines model virtual worlds as collections of isolated data structures sitting in empty coordinate space. Objects remain inert until an explicit collision check or event bus schedules an imperative callback, requiring brittle $O(N^2)$ state scripts to coordinate cross-entity behavior. **Dzog** proposes an alternative paradigm: *relation-first computing* over a continuous physical field. Drawing on David Bohm and Basil Hiley's process-first formulation of quantum mechanics (*active information*) and James Hillman's concept of the *situated image*, Dzog treats the spatial medium itself as the primary computational substrate. Every actor—whether a projectile, a destructible crystal asteroid, a shield, or an autonomous swarm—is a persistent pattern of wave excitation and material commitment within a shared Hamiltonian lattice. Because the medium mediates all causality, collision response, structural fracture, AI navigation, and real-time audio synthesis emerge directly from local physical laws rather than special-case scripts.

---

## 1. The Architectural Problem: Why Move Beyond Object Isolation?

### 1.1 The Limits of Object-Centric Engines
To a computer scientist, the standard architecture of an interactive simulation is familiar: a scene is a list of structs (e.g., `struct Asteroid { pos: Vec2, hp: 3 }`) updated in a loop. Space is merely a coordinate system—a void with no state, memory, or tension.

While efficient for sparse rigid bodies, this object-centric model creates three fundamental bottlenecks when building rich, emergent worlds:
1. **Dead Space**: Nothing exists between objects. A near-miss carries zero physical consequence unless a developer manually codes a proximity trigger.
2. **Combinatorial Scripting ($O(N^2)$ Complexity)**: Every new interaction—a laser heating a shield, a shockwave cracking a weakened wall, a drone weaving around debris—requires bespoke glue code linking specific object pairs.
3. **Binary Destruction**: Damage is typically reduced to an integer counter (`hp -= 1`) swapped for a pre-baked explosion animation, disconnecting visual appearance from structural reality.

### 1.2 The Field-First Inversion
What if the grid of space itself did the computing?

Just as a raster display uses a 2D array of *pixels* to represent color, Dzog uses a 2D array of coupled **field cells** to represent physical state and material properties. In this architecture:
- **Space is an active medium**: Every cell continuously stores pressure ($p$), velocity ($\mathbf{v}$), density ($\rho$), stiffness ($K$), damping ($\gamma$), and structural cohesion ($C$).
- **Objects are patterns in the field**: A solid ship or asteroid is simply a region of cells with high cohesion and material density; a laser bolt is a concentrated packet of wave energy traveling through those cells.
- **Causality is local and automatic**: Instead of checking whether `Projectile A` intersects `Object B`, every cell updates itself based on its immediate neighbors. Reflection, recoil, fracture, and sound emerge naturally from waves crossing material boundaries.

---

## 2. Solitary Waves (Solitons): Turning Waves into Projectiles

### 2.1 Plain Explanation
If you drop a pebble into a pond, the ripple spreads outward in a circle and quickly fades away. In physics, this is called *dispersion*.

### 2.2 Why It Matters to the Engine
If we want to build an engine without rigid-body projectile structs—where a plasma bolt or a Pong ball is literally a wave traveling through the grid—standard linear wave math fails. A linear wave pulse spreads out radially and loses its punch before reaching its target. We need a way for a wave packet to pull itself together as it travels, acting like a solid particle while remaining a pure wave.

### 2.3 Technical Details: The Saturating Soliton Law
In physics, a wave packet that balances dispersion with self-focusing is called a **soliton**. Standard textbook equations for solitons (such as cubic Kerr nonlinearity, $\beta p^3$) have a fatal flaw for real-time software: when two large waves collide, the cubic term explodes to infinity (`NaN`), crashing the simulation.

Dzog solves this by adding a **bounded, rational nonlinear term** $f_{\text{nl}}(p)$ to the standard acoustic wave equations:

$$\rho(\mathbf{x}) \frac{\partial \mathbf{v}}{\partial t} = -\nabla p - \rho(\mathbf{x})\gamma(\mathbf{x})\mathbf{v}$$

$$\frac{\partial p}{\partial t} = -K(\mathbf{x})\nabla \cdot \mathbf{v} - \gamma(\mathbf{x})p + f_{\text{nl}}(p) + s(\mathbf{x}, t)$$

where the nonlinear self-focusing law is defined as:

$$f_{\text{nl}}(p) = \frac{\beta \, p}{1 + \alpha \, p^2} - \kappa \, p$$

Each parameter serves a distinct systems engineering goal:

| Parameter | Plain Meaning | Role in the Runtime |
| :--- | :--- | :--- |
| **$\beta > 0$** | **Self-Focusing Gain** | Pulls high-energy wave peaks inward so projectiles hold a crisp, particle-like shape across the screen. |
| **$\alpha > 0$** | **Safety Ceiling (Saturation)** | Caps maximum amplification at $\frac{\beta}{2\sqrt{\alpha}}$ as $p \to \infty$, mathematically guaranteeing the solver never blows up into `NaN`s when shockwaves collide. |
| **$\kappa > 0$** | **Noise Gate (Vacuum Scrubbing)** | Dampens tiny leftover ripples below the soliton threshold so empty space stays clean and quiet. |

---

## 3. Pilot-Wave Guidance ($Q$): Emergent Navigation Without Pathfinding

### 3.1 Plain Explanation
Normally, waves push things through brute force—a bigger wave shoves a floating object harder. In the 1950s, physicists David Bohm and Basil Hiley highlighted a second, subtler way waves guide matter in quantum mechanics: through the **shape (curvature)** of the wave rather than its raw height. They called this *active information*. Just as a radio signal guides an autonomous aircraft not by physically pushing the wings, but by the *form* of the wave carrying information, the geometry of a wavefield can steer trajectories.

### 3.2 Why It Matters to the Engine
In traditional games, making an alien squadron fly in tight geometric formations or weave around incoming fire requires A* pathfinding graphs, steering raycasts, or hardcoded spline curves.

By computing Bohm and Hiley's **Quantum Potential ($Q$)** on the GPU, the wavefield itself becomes a dynamic navigation mesh. A mothership can radiate a faint, patterned standing wave (like sand patterns on a vibrating Chladni plate), and lightweight swarm drones will automatically slide into the wave's geometric pockets and surf alongside the carrier—wholly emergent from field math, with zero pathfinding scripts.

### 3.3 Technical Details: The Bohm-Hiley Quantum Potential
From the regularized wave amplitude envelope $R(\mathbf{x}, t) = \sqrt{p(\mathbf{x}, t)^2 + \epsilon^2}$, the engine computes the scalar quantum potential $Q(\mathbf{x}, t)$:

$$Q(\mathbf{x}, t) = -\frac{\nabla^2 R(\mathbf{x}, t)}{R(\mathbf{x}, t)} \quad (\epsilon = 0.08)$$

Notice that $R$ appears in both the numerator and the denominator:
- **Scale-Invariance**: Because $\nabla^2 R$ is divided by $R$, the magnitude of $Q$ depends on *spatial curvature* (where interference ridges and valleys bend sharply), not raw wave amplitude.
- **Dual Force Law**: Every field-coupled actor integrates both classical radiation pressure ($-\nabla p$, brute-force shove) and pilot-wave guidance ($-\nabla Q$, geometric steering), alongside long-range gravity ($-\nabla \Phi_g$):
  $$\mathbf{F}_{\text{total}} = -\kappa_p \nabla p - \kappa_Q \nabla Q - m \nabla \Phi_g$$
- **Chladni Swarm Carriers (`ChladniCarrierEmitter`)**: Cruisers emit multi-lobed standing waves $p_{\text{carrier}}(r, \theta, t) = A (1 - r/R_0)^2 \cos(k_r r - \phi)\cos(m \theta)$. Swarm drones experience $-\nabla Q$ attraction into the nodal valleys ($Q < 0$), naturally orbiting and reforming around obstacles as the carrier wave bends.

---

## 4. Material Impedance, Topological Cleaving & Annealing

### 4.1 Plain Explanation
In the real world, objects don't have a single "health bar." A block of quartz, a slab of volcanic glass, and a soft sponge react differently when struck because their internal density and stiffness differ. When a crystal shatters, it breaks along physical crack lines where stress concentrates—and if it is repaired, new crystal grows back onto the surviving structure.

### 4.2 Why It Matters to the Engine
Instead of writing custom material scripts (`if target.type == Metal { reflect_bullet() }`) or tracking integer hitpoints (`hp: 3`), Dzog assigns every cell four physical properties: **Density ($\rho$)**, **Bulk Modulus / Stiffness ($K$)**, **Damping ($\gamma$)**, and **Cohesion ($C \in [0, 1]$)**.
- **Automatic Material Behavior**: Wave speed and reflection ratios arise automatically from $\rho$ and $K$.
- **True Structural Fracture**: Lasers and shockwaves erode cohesion $C$ at the exact cells they strike, carving glowing internal cracks that guide subsequent waves until the object physically splits in two.
- **Topological Ships & Healing**: In the `shmup` demo, the player's ship has three physical crystal lobes (Port Wing, Core, Starboard Wing) instead of HP. Taking a heavy hit on the left wing severs that wing physically; catching a piezoelectric "Power Cube" triggers **annealing** ($\Delta C > 0$), regrowing the lost crystal wing cell-by-cell.

### 4.3 Technical Details: Impedance & Connected-Component Cleaving

```text
   [ Constitutive Material Grid: ρ(x), K(x), γ(x), C(x) ]
          ^                                      |
          | Erosion (ΔC < 0) / Annealing (ΔC > 0)| Fresnel Impedance Z = √(ρK)
          | Moving Hull Work (v_n = v_actor)     | & Wave Speed c = √(K/ρ)
          |                                      v
   [ Situated Actors ] <---------------- [ Active Field: p, v, Q, L ]
                        Guidance: F = -κ_p ∇p - κ_Q ∇Q - m ∇Φ_g
```

1. **Acoustic Impedance & Fresnel Reflections**:
   Each material defines a local wave speed $c(\mathbf{x}) = \sqrt{K(\mathbf{x})/\rho(\mathbf{x})}$ and characteristic impedance $Z(\mathbf{x}) = \sqrt{\rho(\mathbf{x}) K(\mathbf{x})}$. When a wave crosses from medium $Z_1$ into $Z_2$, reflection ($R$) and transmission ($T$) follow exact Fresnel mechanics without branching logic:
   $$R = \frac{Z_2 - Z_1}{Z_2 + Z_1}, \qquad T = \frac{2 Z_2}{Z_2 + Z_1}$$
   Dense `ObsidianAnvil` ($Z = 2.02$) naturally bounces glancing laser bolts, while `PiezoQuartz` ($c = 1.48$) channels fast internal stress waves.
2. **Moving Boundaries & Antiphase Energy Harvesting**:
   Moving hulls stamp their instantaneous velocity ($v_n = v_{\text{actor}}$) into cell-face masks, compressing waves ahead of them (Doppler shift). Firing an **Antiphase Shield** injects a $\Delta \phi = \pi$ inverted wavefront that destructively cancels incoming enemy fire and harvests the annihilated Hamiltonian wave energy into the ship's weapon capacitor.
3. **BFS Topological Cleaving & Annealing**:
   When local overpressure $|p|$ exceeds a body's yield threshold, $C(\mathbf{x})$ erodes along crystallographic stress axes. Once a fracture line reaches $C = 0$ across a body, a Breadth-First Search (BFS) connected-component pass detects the topological split and spawns independent tumbling child bodies conserving linear and angular momentum. Conversely, `CohesiveBody::anneal_cells` restores $C \to 1$ outward from surviving core cells.

---

## 5. Real-Time Numerical Engineering & GPU Pipeline

### 5.1 Plain Explanation
Simulating partial differential equations (PDEs) across tens of thousands of cells at 60–120 frames per second is computationally demanding. Furthermore, naive grid math suffers from visual "checkerboard" artifacts, numerical explosions, and slow data transfers between the graphics card (GPU) and main processor (CPU).

### 5.2 Why It Matters to the Engine
For a field engine to be viable for fast-paced games, it must satisfy three engineering constraints:
1. **Unconditional Stability**: It must never crash with `NaN` values, even during chaotic multi-ship battles.
2. **Zero-Readback GPU Speed**: The CPU cannot afford to download the entire field texture from the GPU every frame just to figure out how much force is pushing on a ship.
3. **Deterministic Snapshots**: Players and developers should be able to save the exact state of the world at any frame, test a "what-if" action, and rewind instantly.

### 5.3 Technical Details: Yee Staggering & Zero-Readback WGPU Compute

#### Staggered Grid Discretization
Dzog uses a **Yee-staggered finite-difference time-domain (FDTD)** lattice (full derivations in [`docs/field_math.md`](./field_math.md)):
- **Cell Centers $(i+\frac{1}{2}, j+\frac{1}{2})$ at integer times $t^n$**: Pressure $p$, Bohmian potential $Q$, luminescence $L$, and material properties $(\rho, K, \gamma, C)$.
- **East/North Cell Faces at half-integer times $t^{n+1/2}$**: Velocities $v_x, v_y$ and moving wall masks.
- **Exponential Damping**: Energy dissipation integrates via $\exp(-\gamma \Delta t)$ rather than Euler subtraction $(1 - \gamma \Delta t)$, ensuring damping factors remain strictly positive under any time step $\Delta t$ bounded by the global Courant–Friedrichs–Lewy (CFL) limit.

#### GPU Texture Layout & Compute Passes (`dzog-wgpu`)
To maximize GPU memory bandwidth, field states are packed into cache-coherent `RGBA32Float` textures:

| GPU Texture | `.r` Channel | `.g` Channel | `.b` Channel | `.a` Channel |
| :--- | :--- | :--- | :--- | :--- |
| **`State` (Ping-Pong)** | Pressure $p$ | East Velocity $v_x$ | North Velocity $v_y$ | Bohmian Potential $Q$ |
| **`Material`** | Density $\rho$ | Bulk Modulus $K$ | Damping $\gamma$ | Cohesion $C$ |
| **`Boundary`** | East Wall Mask | East Wall $v_x$ | North Wall Mask | North Wall $v_y$ |
| **`Luminescence`** | Exciton Glow $L$ | *Unused* | *Unused* | *Unused* |

```text
[ CPU: Body Kinematics & Impulses ]
                 |
                 v  (Compact Uniform Upload)
+-----------------------------------------------------------------------+
| WGPU Compute Pipeline (dzog-wgpu)                                     |
|  1. impulse.wgsl      -> Stamp moving hulls, plumes & carrier pulses  |
|  2. velocity.wgsl     -> Update staggered vx, vy & Bohmian Q          |
|  3. pressure.wgsl     -> Update p + rational soliton law f_nl(p)      |
|  4. luminescence.wgsl -> Evolve persistent exciton trail glow L       |
|  5. reduce_body.wgsl  -> Parallel reduction of -∇p, -∇Q & audio probes|
+-----------------------------------------------------------------------+
                 |                                      |
                 v (Zero-Readback Texture)              v (Compact Struct Readback)
[ Bevy Perceptual Shaders (dzog-bevy) ]     [ Next Frame Body Update & 48 kHz Audio ]
```

Instead of copying megabytes of field textures back to the CPU, `reduce_body.wgsl` performs a parallel reduction on the GPU over each body's bounding contour, returning only a tiny array of net forces, torques, and acoustic probe samples (up to 128 bodies).

---

## 6. Perceptual Projections: Seeing and Hearing the Same Field

### 6.1 Plain Explanation
In most games, graphics and sound are separate illusions: hitting an asteroid plays a pre-recorded `explosion.wav` file while spawning a particle sprite. In Dzog, following James Hillman's concept of the *situated image*, visuals and audio are simply two different ways of perceiving the exact same underlying field math.

### 6.2 Why It Matters to the Engine
Because the lattice already computes real physical pressure oscillations and material wave speeds:
- **Multiple Trustworthy Visual Lenses**: Developers and players can toggle between the artistic game view and scientific diagnostic views (inspecting invisible pilot-wave channels $Q$ or material stress $C$) at any time without altering gameplay.
- **Zero `.wav` Files Required**: Virtual microphones attached to the player's ship sample the pressure waves hitting the hull and drive a physical synthesizer tuned to the exact mineral stiffness of whatever object was just struck.

### 6.3 Technical Details: Multi-View Rendering & $48\text{ kHz}$ Modal Sonification
1. **Perceptual View Projections (`FieldViewMode`)**:
   - *Composite Experience View*: Renders 3D-faceted crystalline bodies dynamically lit by local pressure gradients ($\nabla p$) and exciton luminescence ($\nabla L$).
   - *Bohmian & Pressure View ($p, Q$)*: Exposes the pilot-wave guidance valleys steering AI trajectories.
   - *Velocity & Poynting Flux View ($\mathbf{v}, \mathbf{I} = p\mathbf{v}$)*: Visualizes instantaneous energy transport vectors.
   - *Impedance & Cohesion View ($Z, C$)*: Displays material commitments and internal fracture waveguides.
2. **Material-Tuned Modal Audio (`FieldModalSynthesizer`)**:
   Detailed in [`docs/overview_audio.md`](./overview_audio.md), `BinauralCockpitSonifier` samples port and starboard wingtip pressures and Doppler closing speeds at $60\text{ Hz}$, feeding a lock-free $48\text{ kHz}$ Bevy `Decodable` stream (`DzogFieldAudioSource`). Strike frequencies derive directly from the struck material's acoustic wave speed:
   $$f_0 = 440 \, c(\mathbf{x}) = 440 \sqrt{\frac{K(\mathbf{x})}{\rho(\mathbf{x})}}$$
   High-stiffness `PiezoQuartz` rings with bright crystalline harmonics, while dense `ObsidianAnvil` produces low metallic clangs. Sub-threshold vacuum ripples ($|p| < 0.15$) are gated to exact `(0.0, 0.0)` silence.

---

## 7. Crate Architecture & Verification Suite

The workspace enforces a strict separation between pure field physics (`dzog-core`), GPU compute shaders (`dzog-wgpu`), host integration (`dzog-bevy`), and executable simulations:

| Crate | Architectural Role |
| :--- | :--- |
| [`crates/dzog-core`](../crates/dzog-core) | Host-agnostic Rust reference solver: Yee FDTD stencils, soliton law, Bohmian $Q$, Poisson gravity $\Phi_g$, 3D-faceted `CohesiveBody` rasterizer & BFS cleaving, $48\text{ kHz}$ modal DSP, and deterministic RON `Checkpoint` branching. |
| [`crates/dzog-wgpu`](../crates/dzog-wgpu) | Hardware WGPU compute pipeline (`velocity.wgsl`, `pressure.wgsl`, `luminescence.wgsl`, `impulse.wgsl`, `reduce_body.wgsl`) with verified CPU/GPU numerical agreement ($< 10^{-4}$). |
| [`crates/dzog-bevy`](../crates/dzog-bevy) | Bevy engine plugin (`DzogBevyPlugin`) managing windowing, input, `FieldViewMode` canvas presentation, `GpuFieldAccelerator`, and native `bevy_audio` sink playback. |
| [`pong`](../pong) | **Solid-State & Quantum-Vacuum Breakout**: Demonstrates pure solitary wave balls (zero rigid-body structs), moving boundary Doppler paddles, and sympathetic crystal cleavage cascades. |
| [`asteroids`](../asteroids) | **Crystalline Asteroids & Alien Ecology**: Toroidal & gravity-well wave mechanics, multi-mineral 3D-faceted asteroids, antiphase shield energy harvesting, and pilot-wave alien craft. |
| [`crates/shmup`](../crates/shmup) | **Crystalline Corridor**: Vertical metamaterial canyon shooter featuring a topological **Tri-Lobe Crystalline Ship**, spline-free **Chladni Pilot-Wave Swarms**, and contested **Annealing Power Cubes** ($\Delta C > 0$). |

### Verification Commands

All physical invariants—soliton self-cohesion, Fresnel reflection ratios, energy conservation, topological BFS cleaving, reversible annealing, and GPU/CPU equivalence—are verified via automated tests:

```bash
# Run all 36 analytical, topological, and GPU/CPU numerical equivalence tests
cargo test --workspace

# Run headless WGPU zero-readback throughput benchmarks
cargo run -p asteroids -- --bench
cargo run -p shmup -- --bench
cargo run -p pong -- --bench
```
