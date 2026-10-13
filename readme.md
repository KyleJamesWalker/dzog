# Dzoglang

**A continuous field-mechanics runtime for relation-first computing.**

Conventional runtimes decompose reality into isolated object graphs, discrete state machines, and imperative instruction narratives. **Dzoglang** replaces object-centric execution with a spatially continuous lattice governed by partial differential equations (PDEs), nonlinear solitary wave dynamics, and Bohm-Hiley active-information guidance.

In Dzoglang, a program is not a sequence of instructions attached to objects—it is an **executable situation**: a Hamiltonian landscape of constitutive laws, heterogeneous material fields, boundary commitments, and excitations. Persistent entities (solitons, waveguides, structural bodies) emerge from local field relations, interfere across space, and exchange momentum without scene graphs or collision hierarchies.

Read [philosophy.md](./philosophy.md) for the theoretical foundations (Hillman's situated image, Bohm-Hiley quantum potential, Dzogchen nonconceptual encounter) and [docs/ARCHITECTURE.md](./docs/ARCHITECTURE.md) for the runtime architecture.

---

## Theoretical Foundation: Field Mechanics as Computation

Dzoglang operationalizes concepts from quantum field theory, nonlinear wave mechanics, and Bohmian pilot-wave dynamics into a deterministic GPU/CPU lattice runtime:

1. **Constitutive Field Laws over Object Scripts**:
   Every cell evolves under a unified staggered finite-difference time-domain (FDTD) operator. Behavior is authored by shaping material density $\rho(\mathbf{x})$, bulk modulus $K(\mathbf{x})$, dissipation $\gamma(\mathbf{x})$, and structural cohesion $C(\mathbf{x}) \in [0, 1]$.
2. **Nonlinear Solitary Cohesion (Solitons)**:
   Linear waves disperse radially ($1/\sqrt{r}$). Dzoglang introduces a saturating nonlinear self-focusing potential that balances numerical dispersion, allowing localized excitations to persist and rebound as coherent quasi-particles without rigid-body wrappers.
3. **Bohm-Hiley Quantum Potential & Active Information**:
   Following Bohm and Hiley's second-order formulation, the runtime computes the quantum potential $Q(\mathbf{x}) = -\nabla^2 R(\mathbf{x}) / R(\mathbf{x})$ from the regularized field envelope $R(\mathbf{x}) = \sqrt{p(\mathbf{x})^2 + \epsilon^2}$. Because $Q$ depends on *spatial curvature* (form) rather than raw amplitude, standing-wave channels and interference ridges exert an organizing guidance force $\mathbf{F}_Q = -\nabla Q$ on trajectories.
4. **Continuous Phase Transitions & Structural Erosion**:
   Solid structures are not boolean bounding boxes; they are regions of high cohesion $C(\mathbf{x})$ with finite yield thresholds $p_{\text{yield}}(\mathbf{x})$. Acoustic overpressure ($|p| > p_{\text{yield}}$) erodes cohesion, opening semi-permeable fracture waveguides before triggering cavitation phase transitions back to the ambient fluid medium.
5. **Counterfactual Situation Branching**:
   Authoring and verification follow the cycle:
   ```text
   Encounter -> Perturb -> Compare -> Commit -> Compose
   ```
   Complete staggered field states can be checkpointed at any integer step boundary, branched with modified relational laws, and replayed to measure causal divergence.

---

## Governing Equations & Discretization

The runtime evolves scalar pressure $p(\mathbf{x}, t)$ and particle velocity $\mathbf{v}(\mathbf{x}, t)$ on a 2D Yee-staggered lattice ($p, \rho, K, \gamma, C$ at cell centers; $v_x$ on East faces; $v_y$ on North faces; velocity staggered at half-time steps $t \pm \Delta t/2$):

$$\rho(\mathbf{x}) \frac{\partial \mathbf{v}}{\partial t} = -\nabla p - \rho(\mathbf{x})\gamma(\mathbf{x})\mathbf{v}$$

$$\frac{\partial p}{\partial t} = -K(\mathbf{x})\nabla \cdot \mathbf{v} - \gamma(\mathbf{x})p + f_{\text{nl}}(p) + s(\mathbf{x}, t)$$

where the saturating nonlinear solitary flux and local wave impedance are:

$$f_{\text{nl}}(p) = \frac{\beta \, p}{1 + \alpha \, p^2} - \kappa \, p, \qquad c(\mathbf{x}) = \sqrt{\frac{K(\mathbf{x})}{\rho(\mathbf{x})}}, \qquad Z(\mathbf{x}) = \sqrt{\rho(\mathbf{x}) K(\mathbf{x})}$$

### Numerical Commitments

- **Interface Stencils**: Face densities derive from the arithmetic mean of adjacent cells, $\rho_{\text{face}}^{-1} = 2 / (\rho_L + \rho_R)$, yielding verified Fresnel reflection/transmission ratios $R = (Z_2 - Z_1)/(Z_2 + Z_1)$.
- **Exponential Dissipation**: Losses integrate via unconditionally nonnegative decay factors $\exp(-\gamma \Delta t)$.
- **Global CFL Stability Envelope**: The time step $\Delta t$ is locked prior to execution against the global material envelope:
  $$\Delta t \le \frac{\text{safety}}{c_{\text{bound}} \sqrt{\Delta x^{-2} + \Delta y^{-2}}}, \qquad c_{\text{bound}} = \sqrt{\frac{K_{\max}}{\rho_{\min}}}, \qquad 0 < \text{safety} < 1$$
- **Hamiltonian Energy Accounting**: Instantaneous compressional potential energy $\int \frac{p^2}{2K}\,dA$, kinetic energy $\int \frac{1}{2}\rho |\mathbf{v}|^2\,dA$, and acoustic Poynting flux $\mathbf{I} = p\mathbf{v}$ are tracked continuously to verify conservation and bounded drift.

---

## Workspace Architecture

| Crate | Responsibility |
| --- | --- |
| `crates/dzog-core` | Reference Yee-staggered FDTD solver, nonlinear soliton law, Bohmian quantum potential $Q(\mathbf{x})$, heterogeneous material & cohesion fields, moving boundary conditions, Hamiltonian energy accounting, pressure probes, offline WAV synthesis, and RON `FieldSpec` / `Checkpoint` serialization. |
| `crates/dzog-wgpu` | GPU-native compute pipeline (`velocity.wgsl`, `pressure.wgsl`, `render.wgsl`) over ping-pong `RGBA32Float` state textures, material/boundary textures, and asynchronous staging readback with verified CPU/GPU agreement ($< 10^{-4}$). |
| `crates/dzog-app` | Standalone `winit` + `wgpu` interactive acoustic lattice instrument for real-time GPU wave excitation and boundary inspection. |
| `pong` | **Field-First Soliton Breakout**: Bevy 0.15 interactive demonstration of wave-matter coupling, Bohmian + radiation-pressure guidance, point-of-impact structural fracturing, acoustic resonance cleaving, and counterfactual checkpoint branching. |

### GPU Compute Pipeline (`dzog-wgpu`)

```text
validated FieldSpec / step-indexed interventions
          |
          v
Rust scheduler -> WGPU compute substeps (Pass 1: Velocity A->B, Pass 2: Pressure B->A)
                                      |                             |
                                      v                             v
                               pressure probes               fullscreen shader
                                      |
                                      v
                        staging readback -> audio / inspector
```

| GPU Resource | Format | Packed Channels |
| --- | --- | --- |
| `State A / B` | `RGBA32Float` | `(p_center, vx_east, vy_north, _)` |
| `Material` | `RGBA32Float` | `(density_rho, bulk_modulus_K, damping_gamma, _)` |
| `Boundary` | `RGBA32Float` | `(wall_east_mask, wall_vx, wall_north_mask, wall_vy)` |

---

## Running the Runtime & Demos

```bash
# Run the full verification suite (21 unit, analytical gate, GPU/CPU agreement, and physics tests)
cargo test --workspace

# Launch the Field-First Soliton Breakout demo (Bevy 0.15 GUI)
cargo run -p pong

# Run the headless FDTD solver & rasterizer throughput benchmark
cargo run -p pong -- --bench

# Launch the native WGPU compute field instrument
cargo run -p dzog-app
```

---

## Artifacts: Image As Program

A screenshot or color bitmap is not a program. Dzoglang distinguishes three serializable artifacts (`dzog-core::manifest`):

- **Specification (`FieldSpec`)**: Authored constitutive laws, grid scale, material bounds, boundary conventions, and probe topologies.
- **Checkpoint (`Checkpoint`)**: A complete, bit-reproducible snapshot of all staggered dynamic fields ($p^n, \mathbf{v}^{n-1/2}$), material/cohesion layers, and step clocks at an integer step boundary.
- **Performance**: A specification or checkpoint paired with step-indexed interventions for deterministic replay and counterfactual comparison.

---

## Platform Roadmap & Research Directions

1. **Multi-Component Spinor & Complex Wavefields**: Extending the real scalar-vector lattice $(p, \mathbf{v})$ to complex-valued Gross-Pitaevskii / nonlinear Schrödinger condensates ($\psi \in \mathbb{C}$) supporting quantized vortices and topological defects.
2. ** Elastodynamic Stress Tensors**: Expanding scalar bulk modulus $K$ and cohesion $C$ into full 2D Cauchy stress and shear strain tensors $(\sigma_{xx}, \sigma_{yy}, \sigma_{xy})$ for anisotropic wave propagation and plastic deformation.
3. **Gauge-Coupled Potentials**: Introducing synthetic vector potentials $\mathbf{A}(\mathbf{x}, t)$ on lattice links (Peierls substitution) to model Lorentz-like chiral wave deflection and non-reciprocal metamaterials.
4. **Asynchronous GPU-to-Audio Sonification**: Completing the lock-free SPSC ring pipeline from GPU probe storage buffers to `cpal` at 48 kHz (`dt_sample = a / Fs`), unifying visual and audible observation of the same evolving field state.
5. **Situated Composition & Fragment Algebra**: Formalizing boundary impedance matching and scale invariants when composing reusable field fragments across heterogeneous parent landscapes.

---

## Engineering Claims & Boundaries

| Capability | Engineering Boundary |
| --- | --- |
| **GPU-native stencil parallelism** | $O(\text{cells} \times \text{substeps})$ uniform dispatch work; empty regions cost the same as active regions. |
| **Collision-free wave interaction** | Interference and boundary reflection require zero BVH or object pair queries, but do not replace general rigid-body contact constraints. |
| **Continuous field representation** | Finite-difference lattices remain discrete, dispersive at high wavenumbers, and bounded by Nyquist/CFL limits. |
| **Cross-device reproducibility** | Exact within a backend; cross-GPU floating-point associativity requires declared numerical tolerances ($10^{-4}$). |

---

## References

- [Philosophy](./philosophy.md): Conceptual synthesis (Hillman, Bohm-Hiley, Dzogchen) and testable hypotheses.
- [Architecture](./docs/ARCHITECTURE.md): Nonlinear solitary field equations and runtime pipeline details.
- B. J. Hiley, [*Process, Distinction, Groupoids and Clifford Algebras*](https://arxiv.org/abs/1211.2107).
- [Stanford Encyclopedia of Philosophy: Bohmian Mechanics](https://plato.stanford.edu/entries/qm-bohm/).
