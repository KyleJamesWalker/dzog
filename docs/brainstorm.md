# Dzog Engine: Dialectical Critique, Capabilities & Couch Game Proposals

This document synthesizes the architectural evaluation of **Dzog** through a dialectical lens—bridging **John Carmack** (real-time systems realism, CFL/Nyquist limits, GPU bandwidth, player agency) and **David Bohm, Basil Hiley & James Hillman** (active information $Q = -\nabla^2 R / R$, relation-first causality, and the *situated image* vs. detached symbols).

---

## 1. Core Thesis: Situated Image vs. Isolated Symbol

In conventional engines, entities are isolated data structures (`struct Asteroid { pos, hp: 3 }`) sitting in dead coordinate space. Appearance, collision, audio, and AI navigation are stitched together through brittle $O(N^2)$ imperative scripts.

In **Dzog**, the continuous lattice is the primary computational substrate:
- **Bohm–Hiley Active Information ($Q = -\nabla^2 R / R$)**: Because $R$ normalizes $\nabla^2 R$, guidance forces ($\mathbf{F}_Q = -\kappa_Q \nabla Q$) depend on the *spatial curvature (form)* of the wavefield rather than raw amplitude. Standing waves act as dynamic, zero-script navigation meshes.
- **Hillman's Situated Image**: An actor is an excitation coupled to its milieu ($\rho, K, \gamma, C$). Its structural integrity, acoustic pitch ($f_0 = 440\sqrt{K/\rho}$), specular luminescence ($\nabla L$), and wave reflection ($Z = \sqrt{\rho K}$) are projections of a single undivided physical state.

---

## 2. Adversarial Critique: Cold Hard Engineering Truths

1. **The CFL & Resolution Wall**:
   $$\Delta t \le \frac{\text{safety}}{c_{\text{max}} \sqrt{\Delta x^{-2} + \Delta y^{-2}}}, \qquad c_{\text{max}} = \sqrt{\frac{\max K}{\min \rho}}$$
   - Grid doubling in 2D costs $4\times$ cells and $2\times$ substeps ($8\times$ GPU work); 3D costs $16\times$.
   - A single high-stiffness material (`PiezoQuartz`) globally forces smaller $\Delta t$ across all cells, including empty vacuum.
2. **Eulerian–Lagrangian Friction**:
   - Isotropic projectiles work as pure grid solitons ($f_{\text{nl}}(p) = \frac{\beta p}{1+\alpha p^2} - \kappa p$).
   - Complex rotating hulls (`CohesiveBody`) cannot be pure grid excitations on a $240 \times 150$ lattice without numerical dispersion melting their edges; they require hybrid contour rasterization and parallel GPU force reduction (`reduce_body.wgsl`).
3. **Authorial Coupling & Balancing Complexity**:
   - When weapons ($-\nabla p$), AI steering ($-\nabla Q$), shields ($\Delta \phi = \pi$), and fracture ($C$) share one medium, tuning one parameter ripples across all systems.
4. **Verdict on Engine Scope**:
   - **Do not** position Dzog as a general-purpose 3D or symbol-heavy engine replacement.
   - **Do** focus on **single-screen 2D/2.5D arenas and room-based worlds** where the fixed $240 \times 150$ (or $512 \times 320$) lattice matches the shared screen, turning the scaling boundary into a design strength.

---

## 3. Why Couch Play & Co-Op Are the Ideal Medium

Single-screen couch games eliminate scrolling grid-scale bottlenecks while highlighting Dzog's strongest differentiator: **shared-medium causality**.

In traditional co-op, two players on one screen are independent structs until they collide. In Dzog, two players inhabit the **same continuous wavefunction**:
- **Constructive Co-Op ($\Delta \phi = 0$)**: Crossing two phase-locked beams doubles amplitude ($2p$) and quadruples local energy density ($4p^2$), exceeding high yield thresholds ($p_{\text{yield}}$) that neither player can crack alone.
- **Destructive Shielding ($\Delta \phi = \pi$)**: One player can project an antiphase wavefront to cancel a boss shockwave hitting their partner, harvesting the annihilated Hamiltonian energy ($\Delta H = \Delta E_p + \Delta E_k$) into shared weapon charge.
- **Harmonic Tethering ($-\nabla Q$)**: Interfering carrier emitters between two ships create standing-wave valleys ($Q < 0$) in the space between them—forming an invisible tractor net that traps Annealing Power Cubes or herds friendly swarms.

---

## 4. Rethinking Classic Couch Genres for Dzog

**Design Rule**: *If a mechanic can be faked with a rigid-body sprite and a hitbox, cut it. Keep only mechanics driven by $p, \mathbf{v}, Q, Z,$ and $C$.*

### Proposal A: Interferometer Co-Op (*Asteroids* / *Shmup* Evolution)
Move beyond "shooting rocks with wave bullets" to making **the wavefield between two players** the primary tool:
1. **Cross-Beam Resonance Cleaving**: Dense `ObsidianAnvil` armor reflects single shots via Fresnel impedance ($R = \frac{Z_2 - Z_1}{Z_2 + Z_1}$). Players must triangulate and cross their soliton streams on a single crystal fault line to spike overpressure and trigger a topological BFS cleave.
2. **The 2-Player Chladni Basket**: Flying in formation creates a standing-wave pocket between hulls. Players catch and carry piezoelectric condensates ("Power Cubes") inside this $-\nabla Q$ valley to anneal ($\Delta C > 0$) severed ship wings. Flying too far apart snaps the wave tether.
3. **Acoustic Wake Surfing & Deflection**: Players use thruster boundary work ($v_n = v_{\text{actor}}$) and antiphase pulses to deflect incoming solitons or boost a partner's projectile speed via Doppler compression.

### Proposal B: Asymmetric Field Melee (*Star Control II* Super Melee)
A 1v1 or 2v2 couch arena in a wrap-around toroidal lattice where each ship archetype embodies a specific term of the governing PDEs:

| Ship Archetype | Governing Field Law | Tactical Role |
| :--- | :--- | :--- |
| **Basalt Juggernaut** | **Impedance & Boundary Work** ($\rho \gg 1, K \gg 1, v_n$) | Fires no projectiles. Uses massive Doppler bow-wakes to shove enemy fire back and rams crystals along weak cleavage planes. |
| **Chladni Broodmother** | **Bohmian Guidance** ($Q = -\nabla^2 R / R$) | Emits multi-lobed standing waves (`ChladniCarrierEmitter`). Autonomous fighters surf its $-\nabla Q$ valleys; shifts harmonic modes ($m = 2 \to 6$) to sling swarms at foes. |
| **Piezo Architect** | **Material Seeding & Cohesion** ($\Delta C > 0$) | Deposits rapid-crystallizing quartz into vacuum—building physical cover, $45^\circ$ Fresnel mirrors to bounce shots around corners, or resonant cages. |
| **Phase Siren** | **Antiphase & Modal Resonance** ($\Delta \phi = \pi, f_0 \propto \sqrt{K/\rho}$) | Absorbs enemy solitons via antiphase cancellation to regrow lobes, then emits a continuous wave tuned to the enemy hull's exact $K/\rho$ resonant frequency to shatter it internally. |

### Proposal C: Resonant Dungeon Crawler (*2D Zelda* / *Metroid* Rooms)
Partitioning the world into discrete, single-screen rooms ($240 \times 150$ each) completely solves global CFL and memory scaling while turning every room into a **coupled acoustic-crystalline puzzle box**:
1. **Resonance Locks Instead of Keys**: Heavy bismuth doors open only when players strike a quartz Tuning Pillar and route the wave through cracked floor waveguides ($0.15 \le C < 0.55$) so pulses arrive at dual locks *in phase*.
2. **Co-Op Mirror Routing**: Player 1 angles a high-impedance crystal shield as a Fresnel mirror; Player 2 fires a soliton beam off Player 1's shield to strike a boss's shielded weak spot or ignite a distant luminescence beacon ($L$).
3. **Situated Material Ecology**: Flooding a room with viscous fluid ($\gamma, \rho$) alters wave speeds, weapon pitches, and $-\nabla Q$ channels globally; cleaving a load-bearing crystal pillar drops it into the fluid, permanently reshaping the room's acoustics and navigation.

---

## 5. Prioritized Engine Upgrades (`dzog-core` & `dzog-wgpu`)

### Immediate Upgrades (High Impact, Low Complexity for Couch Co-Op)
1. **Per-Emitter Phase & Frequency Control (`impulse.wgsl`)**:
   - Support explicit phase offsets ($\Delta \phi \in [0, \pi]$) and harmonic mode tags per player emitter to enable intentional constructive cross-beams and antiphase partner shielding.
2. **Dynamic Material Seeding (`pressure.wgsl` / `Material` texture)**:
   - Extend cohesion updates beyond hull erosion/annealing so projectiles and tools can write $(\rho, K, \gamma, C)$ cells directly into empty grid space (enabling the *Piezo Architect* and dungeon waveguide carving).
3. **Co-Op Tether Probe Reduction (`reduce_body.wgsl`)**:
   - Sample $Q$ and $\nabla L$ along the inter-player vector to drive controller haptics, audio harmonics, and visual tether glow when two ships lock into a standing-wave formation.

### Medium-Term Core Solver Upgrades
4. **Dual-Band Complex Envelope ($\psi = R e^{i\theta}$) for Smooth $Q$**:
   - Replace single-real-component $R = \sqrt{p^2 + \epsilon^2}$ (which oscillates at $2\omega$ across wave zero-crossings) with a quadrature pair $(p_r, p_i)$. This yields a rock-steady amplitude envelope $R = \sqrt{p_r^2 + p_i^2}$ for jitter-free $-\nabla Q$ swarm guidance and exact phase angle $\theta$ readout.
5. **Anisotropic Stiffness / Cleavage Axis ($K_{xx}, K_{yy}$)**:
   - Store a directional grain vector in the material state so crystals conduct waves and fracture preferentially along mineral cleavage planes, enabling directional acoustic diodes and waveguides.
6. **Tile-Based Sub-Cycling (Local Time-Stepping)**:
   - Sub-cycle only $16 \times 16$ GPU workgroups containing high-$c$ crystals while stepping vacuum tiles at $1\times$, unlocking $3\text{–}4\times$ higher grid resolution without global CFL slowdowns.
