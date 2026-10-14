# Field-First Computing: A Wave-Matter & Pilot-Wave Runtime Architecture

**Abstract**: Conventional simulation and game engines model virtual worlds as collections of isolated data structures sitting in empty coordinate space. Objects remain inert until an explicit collision check or event bus schedules an imperative callback, requiring brittle $O(N^2)$ state scripts to coordinate cross-entity behavior. **Dzog** implements an alternative paradigm: *relation-first computing* over a continuous physical field. Drawing on David Bohm and Basil Hiley's process-first formulation of quantum mechanics (*active information*) and James Hillman's concept of the *situated image*, Dzog treats the spatial medium itself as the primary computational substrate. Every actor (whether a projectile, a destructible crystal asteroid, a shield, or an autonomous swarm) is a persistent pattern of wave excitation and material commitment within a shared Hamiltonian lattice. Because the medium mediates all causality, collision response, structural fracture, AI navigation, dynamic lighting, and real-time audio synthesis emerge directly from local physical laws rather than special-case scripts.

---

## 1. The Architectural Problem: Why Move Beyond Object Isolation?

### 1.1 The Limits of Object-Centric Engines
To a computer scientist, the standard architecture of an interactive simulation is familiar: a scene is a list of structs (e.g., `struct Asteroid { pos: Vec2, hp: 3 }`) updated in a loop. Space is merely a coordinate system, a void with no state, memory, or tension.

While efficient for sparse rigid bodies, this object-centric model creates three bottlenecks when building rich, emergent worlds:
1. **Dead Space**: Nothing exists between objects. A near-miss has zero physical consequence unless a developer manually codes a proximity trigger.
2. **Combinatorial Scripting ($O(N^2)$ Complexity)**: Every new interaction (a laser heating a shield, a shockwave cracking a weakened wall, or a drone weaving around debris) requires custom glue code linking specific object pairs.
3. **Binary Destruction**: Damage is usually reduced to an integer counter (`hp -= 1`) swapped for a pre-baked explosion animation, disconnecting visual appearance from structural mechanics.

### 1.2 The Field-First Inversion
What if the grid of space itself performed the computation?

Just as a raster display uses a 2D array of *pixels* to represent color, Dzog uses a 2D array of coupled **field cells** to represent physical state and material properties. In this architecture:
- **Space is an active medium**: Every cell stores pressure ($p$), velocity ($\mathbf{v}$), density ($\rho$), stiffness ($K$), damping ($\gamma$), and structural cohesion ($C$).
- **Objects are patterns in the field**: A solid ship or asteroid is a region of cells with high cohesion and material density; a laser bolt is a concentrated packet of wave energy traveling through those cells.
- **Causality is local and automatic**: Instead of checking whether `Projectile A` intersects `Object B`, every cell updates itself from its immediate neighbors. Reflection, recoil, fracture, and sound emerge naturally as waves cross material boundaries.

---

## 2. Solitary Waves (Solitons): Turning Waves into Projectiles

### 2.1 Plain Explanation
If you drop a pebble into a pond, the ripple spreads outward in a circle and fades away. In physics, this spreading is called *dispersion*.

### 2.2 Why It Matters to the Engine
If an engine eliminates rigid-body projectile structs so that a plasma bolt or a Pong ball is literally a wave traveling through the grid, standard linear wave math fails. A linear wave pulse spreads out radially and loses its energy before reaching its target. The wave packet needs a mechanism to pull itself together as it travels, acting like a solid particle while remaining a pure wave.

### 2.3 Technical Details: The Saturating Soliton Law
A wave packet that balances dispersion with self-focusing is called a **soliton**. Standard textbook equations for solitons (such as cubic Kerr nonlinearity, $\beta p^3$) have a fatal flaw for real-time software: when two large waves collide, the cubic term grows without bound (`NaN`), crashing the simulation.

Dzog solves this by adding a **bounded, rational nonlinear term** $f_{\text{nl}}(p)$ to the compressible acoustic wave equations:

$$\rho(\mathbf{x}) \frac{\partial \mathbf{v}}{\partial t} = -\nabla p - \rho(\mathbf{x})\gamma(\mathbf{x})\mathbf{v}$$

$$\frac{\partial p}{\partial t} = -K(\mathbf{x})\nabla \cdot \mathbf{v} - \gamma(\mathbf{x})p + f_{\text{nl}}(p) + s(\mathbf{x}, t)$$

where the nonlinear self-focusing law is defined as:

$$f_{\text{nl}}(p) = \frac{\beta \, p}{1 + \alpha \, p^2} - \kappa \, p$$

Each parameter serves a specific systems engineering goal:

| Parameter | Plain Meaning | Role in the Runtime |
| :--- | :--- | :--- |
| **$\beta > 0$** | **Self-Focusing Gain** | Pulls high-energy wave peaks inward so projectiles hold a compact, particle-like shape across the screen. |
| **$\alpha > 0$** | **Safety Ceiling (Saturation)** | Caps maximum amplification at $\frac{\beta}{2\sqrt{\alpha}}$ as $p \to \infty$, preventing `NaN` blowups when shockwaves collide. |
| **$\kappa > 0$** | **Noise Gate (Vacuum Scrubbing)** | Dampens low-amplitude ripples below the soliton threshold so empty space stays clean and quiet. |

---

## 3. Pilot-Wave Guidance ($Q$): Emergent Navigation Without Pathfinding

### 3.1 Plain Explanation
Normally, waves push objects through brute force: a taller wave shoves a floating body harder. In the 1950s, physicists David Bohm and Basil Hiley highlighted a second way waves guide matter in quantum mechanics: through the **shape (curvature)** of the wave rather than its height. They called this *active information*. Just as a radio beam guides an aircraft not by physically pushing its wings, but through the *form* of the signal, the geometry of a wavefield can steer trajectories.

### 3.2 Why It Matters to the Engine
In traditional games, making an alien squadron fly in tight formations or weave around incoming fire requires A* pathfinding graphs, steering raycasts, or hardcoded spline curves.

By computing Bohm and Hiley's **Quantum Potential ($Q$)** on the GPU, the wavefield itself becomes a dynamic navigation mesh. A mothership can radiate a faint, patterned standing wave (similar to sand patterns on a vibrating Chladni plate), and lightweight swarm drones automatically slide into the wave's geometric pockets and surf alongside the carrier. Formation flight emerges from field math with zero pathfinding scripts.

### 3.3 Technical Details: The Bohm-Hiley Quantum Potential
From the regularized wave amplitude envelope $R(\mathbf{x}, t) = \sqrt{p(\mathbf{x}, t)^2 + \epsilon^2}$, the engine computes the scalar quantum potential $Q(\mathbf{x}, t)$:

$$Q(\mathbf{x}, t) = -\frac{\nabla^2 R(\mathbf{x}, t)}{R(\mathbf{x}, t)} \quad (\epsilon = 0.08)$$

Note that $R$ appears in both the numerator and the denominator:
- **Scale-Invariance**: Because $\nabla^2 R$ is divided by $R$, the magnitude of $Q$ depends on *spatial curvature* (where interference ridges and valleys bend sharply), not raw wave amplitude.
- **Dual Force Law**: Every field-coupled actor integrates both classical radiation pressure ($-\nabla p$, brute-force push) and pilot-wave guidance ($-\nabla Q$, geometric steering), alongside long-range gravity ($-\nabla \Phi_g$):
  $$\mathbf{F}_{\text{total}} = -\kappa_p \nabla p - \kappa_Q \nabla Q - m \nabla \Phi_g$$
- **Chladni Swarm Carriers (`ChladniCarrierEmitter`)**: Cruisers emit multi-lobed standing waves $p_{\text{carrier}}(r, \theta, t) = A (1 - r/R_0)^2 \cos(k_r r - \phi)\cos(m \theta)$. Swarm drones experience $-\nabla Q$ attraction into the nodal valleys ($Q < 0$), orbiting and reforming around obstacles as the carrier wave bends.

---

## 4. Material Impedance, Topological Cleaving & Annealing

### 4.1 Plain Explanation
Physical objects do not have a single "health bar." A block of quartz, a slab of volcanic glass, and a soft sponge react differently when struck because their internal density and stiffness differ. When a crystal shatters, it breaks along crack lines where stress concentrates; when repaired, new crystal grows outward from the surviving structure.

### 4.2 Why It Matters to the Engine
Instead of writing custom material scripts (`if target.type == Metal { reflect_bullet() }`) or tracking integer hitpoints (`hp: 3`), Dzog assigns every cell four physical properties: **Density ($\rho$)**, **Bulk Modulus / Stiffness ($K$)**, **Damping ($\gamma$)**, and **Cohesion ($C \in [0, 1]$)**.
- **Automatic Material Behavior**: Wave speed and reflection ratios follow directly from $\rho$ and $K$.
- **True Structural Fracture**: Lasers and shockwaves erode cohesion $C$ at the exact cells they strike, carving internal cracks that channel subsequent waves until the object splits apart.
- **Topological Ships & Healing**: In the `shmup` demo, the player's ship has three physical crystal lobes (Port Wing, Core, Starboard Wing) instead of hitpoints. Taking a heavy hit on the left wing severs that wing physically; catching a piezoelectric "Power Cube" triggers **annealing** ($\Delta C > 0$), regrowing the lost crystal wing cell by cell.

### 4.3 Technical Details: Impedance, Erosion & Connected-Component Cleaving

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
   Each material defines a local wave speed $c(\mathbf{x}) = \sqrt{K(\mathbf{x})/\rho(\mathbf{x})}$ and characteristic impedance $Z(\mathbf{x}) = \sqrt{\rho(\mathbf{x}) K(\mathbf{x})}$. When a wave crosses from medium $Z_1$ into $Z_2$, reflection ($R$) and transmission ($T$) follow Fresnel mechanics without branching logic:
   $$R = \frac{Z_2 - Z_1}{Z_2 + Z_1}, \qquad T = \frac{2 Z_2}{Z_2 + Z_1}$$
   Dense `ObsidianAnvil` ($Z = 2.02$) reflects glancing laser bolts, while `PiezoQuartz` ($c = 1.48$) transmits fast internal stress waves.
2. **Continuous Cohesion Erosion, BFS Cleaving & Annealing**:
   When local pressure $|p|$ exceeds a material's yield threshold $p_{\text{yield}}$, cohesion erodes proportionally to the overpressure:
   $$C^{n+1} = \max\!\left(0, \; C^n - \eta_{\text{erode}}\left(|p| - p_{\text{yield}}\right)\Delta t\right)$$
   Partially eroded cells ($0.15 \le C < 0.55$) act as internal waveguides that trap and channel subsequent shockwaves along the crack. Once a fracture line reaches $C = 0$ across a body, a Breadth-First Search (BFS) connected-component pass detects the topological split and spawns independent tumbling child fragments conserving momentum. Conversely, capturing an annealing condensate (`CohesiveBody::anneal_cells`) drives $\Delta C > 0$ outward from surviving core cells, sealing cracks and regrowing severed lobes.
3. **Moving Boundaries & Antiphase Energy Harvesting**:
   Moving hulls stamp their instantaneous velocity ($v_n = v_{\text{actor}}$) into cell-face masks, compressing waves ahead of them (Doppler shift). The lattice tracks total **Hamiltonian mechanical energy** ($H = E_p + E_k$) across potential and kinetic components:
   $$E_p = \sum \frac{p^2}{2K} \Delta A, \qquad E_k = \sum \frac{1}{2}\rho |\mathbf{v}|^2 \Delta A$$
   When the player fires an **Antiphase Shield**, it injects an inverted wavefront ($\Delta \phi = \pi$) that destructively cancels incoming enemy fire. The engine measures the resulting drop in local Hamiltonian energy $\Delta H$ and transfers that cancelled wave energy into the ship's weapon capacitor.

---

## 5. Real-Time Numerical Engineering & GPU Pipeline

### 5.1 Plain Explanation
Simulating partial differential equations (PDEs) across tens of thousands of cells at 60 to 120 frames per second is computationally demanding. Naive grid implementations suffer from visual "checkerboard" artifacts, numerical explosions when time steps are too large, and slow data transfers between the GPU and CPU.

### 5.2 Why It Matters to the Engine
For a field engine to run interactive games reliably, it must meet three engineering constraints:
1. **Unconditional Stability**: The solver must never produce `NaN` values, even during chaotic multi-ship battles.
2. **Zero-Readback GPU Speed**: The CPU cannot afford to download the entire field texture from the GPU every frame just to compute the forces acting on a ship.
3. **Deterministic Snapshots**: Players and developers need to save the state of the world at any frame, test a counterfactual action, and rewind cleanly.

### 5.3 Technical Details: Yee Staggering, CFL Stability & Zero-Readback WGPU

#### Staggered Grid Discretization & Stability Bounds
Dzog uses a **Yee-staggered finite-difference time-domain (FDTD)** lattice:
- **Staggered Placement**: Pressure $p$, Bohmian $Q$, luminescence $L$, and material properties $(\rho, K, \gamma, C)$ live at **cell centers** $(i+\frac{1}{2}, j+\frac{1}{2})$ at integer steps $t^n$. Velocities $v_x, v_y$ and moving wall masks live on **East/North cell faces** at half-steps $t^{n+1/2}$. Storing pressure and velocity half a cell apart eliminates odd-even decoupling ("checkerboarding") and preserves exact momentum conservation across solid walls.
- **Global CFL Time-Step Bound**: Information on a discrete grid cannot propagate faster than one cell per step. Before stepping, `SolverConfig::compute_cfl_dt` locks $\Delta t$ against the fastest wave speed across all materials on the grid:
  $$c_{\text{max}} = \sqrt{\frac{\max K}{\min \rho}}, \qquad \Delta t \le \frac{\text{safety}}{c_{\text{max}} \sqrt{\Delta x^{-2} + \Delta y^{-2}}}$$
- **Exponential Damping**: Energy dissipation integrates via $\exp(-\gamma \Delta t)$ rather than Euler subtraction $(1 - \gamma \Delta t)$, keeping damping factors strictly positive inside high-absorption boundary sponges.

#### Zero-Readback GPU Parallel Reduction (`dzog-wgpu`)
To maximize GPU memory bandwidth, field states are packed into cache-coherent `RGBA32Float` ping-pong textures (`State`, `Material`, `Boundary`, and `Luminescence`; see the [`readme.md` Workspace Architecture](../readme.md#workspace-architecture) for channel layouts). Each macro-frame executes five WGSL compute stages:
1. **`impulse.wgsl`**: Stamps moving hulls, thruster plumes, and phase-locked carrier pulses into the grid.
2. **`velocity.wgsl`**: Updates staggered face velocities $v_x, v_y$ and evaluates the Bohmian potential $Q$.
3. **`pressure.wgsl`**: Updates cell-center pressure $p$, applies the rational soliton law $f_{\text{nl}}(p)$, and erodes cohesion $C$.
4. **`luminescence.wgsl`**: Evolves the reaction-diffusion exciton glow field $L$.
5. **`reduce_body.wgsl`**: Performs a parallel reduction on the GPU over each body's bounding contour, returning a compact buffer of net forces ($-\nabla p, -\nabla Q$), torques, and acoustic probe samples (up to 128 bodies) without copying field textures back to the CPU.

---

## 6. Perceptual Projections: Seeing and Hearing the Same Field

### 6.1 Plain Explanation
In conventional engines, graphics, lighting, and sound are separate layers: hitting an asteroid plays a pre-recorded `explosion.wav` file while spawning a particle sprite and a point light. In Dzog, following James Hillman's concept of the *situated image*, visuals, crystal lighting, and audio are different projections of the same underlying field state.

### 6.2 Why It Matters to the Engine
Because the lattice already computes physical pressure oscillations, exciton glow diffusion, and material wave speeds:
- **Field-Driven 3D Crystal Lighting**: Passing laser bolts and stressed crystal cracks emit a persistent luminescence field ($L$) that diffuses through space and illuminates the 3D facets of nearby asteroids.
- **Multiple Diagnostic Lenses**: Developers and players can switch between the composite game view and diagnostic views (inspecting pilot-wave channels $Q$ or material stress $C$) at any time without altering the simulation.
- **Zero `.wav` Files Required**: Virtual microphones attached to the player's ship sample the pressure waves hitting the hull and drive a modal synthesizer tuned to the mineral stiffness of the struck object.

### 6.3 Technical Details: Exciton Luminescence, Multi-View Rendering & Modal Audio
1. **Reaction-Diffusion Exciton Luminescence ($L$)**:
   Pass 3 (`luminescence.wgsl`) evolves a radiative light field $L(\mathbf{x}, t) \in [0, 4]$ driven by wave energy $S_{\text{wave}}(p, \mathbf{v})$ and piezoelectric crystal stress $S_{\text{piezo}}(p, C)$, diffusing at a rate $D \propto c(\mathbf{x})$:
   $$L^{n+1} = e^{-\gamma_L \Delta t}\left(L^n + D \Delta t \, \nabla^2 L^n\right) + \Delta t \left(S_{\text{wave}} + S_{\text{piezo}}\right)$$
   Spatial gradients $\nabla L$ and $\nabla p$ compute per-pixel 3D facet normals on rotating mineral hulls (`CohesiveBody::sample_crystal_facet`), so glowing cracks and passing solitons cast directional specular light onto nearby bodies.
2. **Perceptual View Projections (`FieldViewMode`)**:
   - *Composite Experience View*: Renders 3D-faceted crystalline bodies lit by $\nabla L$ and $\nabla p$.
   - *Bohmian & Pressure View ($p, Q$)*: Exposes the pilot-wave guidance valleys steering AI trajectories.
   - *Velocity & Poynting Flux View ($\mathbf{v}, \mathbf{I} = p\mathbf{v}$)*: Visualizes instantaneous energy transport vectors.
   - *Impedance & Cohesion View ($Z, C$)*: Displays material commitments and internal fracture waveguides.
3. **Material-Tuned Modal Audio (`FieldModalSynthesizer`)**:
   Detailed in [`docs/overview_audio.md`](./overview_audio.md), `BinauralCockpitSonifier` samples port and starboard wingtip pressures and Doppler closing speeds at $60\text{ Hz}$, feeding a lock-free $48\text{ kHz}$ Bevy `Decodable` stream (`DzogFieldAudioSource`). Strike frequencies derive from the struck material's acoustic wave speed:
   $$f_0 = 440 \, c(\mathbf{x}) = 440 \sqrt{\frac{K(\mathbf{x})}{\rho(\mathbf{x})}}$$
   High-stiffness `PiezoQuartz` rings with bright crystalline harmonics, while dense `ObsidianAnvil` produces low metallic clangs. Sub-threshold vacuum ripples ($|p| < 0.15$) are gated to exact `(0.0, 0.0)` silence.

