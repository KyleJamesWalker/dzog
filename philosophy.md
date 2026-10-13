# Dzog Philosophy

## The Proposal

Dzog is a relation-first runtime and programming medium whose substrate is a continuous physical field. Rather than assembling isolated objects and imperative scripts, authors compose a **living ecology of actors** governed by wave mechanics, nonlinear self-cohesion, and quantum-inspired guidance laws.

In conventional game and application engines, entities are sealed state containers sitting in empty coordinate space. They interact only when a broad-phase collider or event bus explicitly schedules a callback. Between discrete events, the world is inert.

In Dzog, the medium itself is alive. Every actor—whether a coherent solitary wave packet (soliton), a semi-permeable membrane, a resonant cavity, or a moving boundary—is coupled to a shared Hamiltonian lattice. Entities radiate wakes, sense distant activity through interference gradients, fracture along continuous stress tensors, and reorganize one another without centralized orchestration.

We do not abolish symbols; we stop making symbolic isolation the starting point.

---

## The Critique of Object Isolation

Symbolic abstraction amputates context when a label (`struct Brick { hp: 2 }`) is treated as a complete account of what it names. Imperative instruction sequences similarly impose a linear narrative onto relationships that exist simultaneously across space.

When an engine models a world as a list of rigid bounding boxes:
- **Space is dead**: Empty coordinates carry no tension, memory, or impedance.
- **Interaction is binary**: Objects either collide at a discrete frame or ignore each other completely.
- **Change is brittle**: Adding a new behavior requires wiring explicit $O(N^2)$ state transitions across object scripts.

The alternative is to ground computation in **coexistence, interference, and field response**. Local constitutive laws generate global organization; objects become persistent patterns of activity within a continuous milieu.

---

## Theoretical Foundations

### 1. Bohm-Hiley Active Information & Quantum Field Mechanics

In quantum field theory (QFT), particles are not independent billiard balls; they are localized excitations (quanta) of underlying fields. David Bohm and Basil Hiley's process-first formulation sharpens this for computation through the **Quantum Potential**:

$$Q(\mathbf{x}, t) = -\frac{\nabla^2 R(\mathbf{x}, t)}{R(\mathbf{x}, t)}$$

where $R$ is the field amplitude envelope. Unlike classical forces that decay purely with amplitude, $Q$ depends on the **normalized spatial curvature (form)** of the field:

- **Active Information**: Form in-forms activity. A subtle standing-wave channel or interference ridge exerts an organizing guidance force $\mathbf{F}_Q = -\nabla Q$ on trajectories, even at low energy density.
- **Mutuality of Actor and Field**: The field guides the actor ($\mathbf{F} = -\nabla p - \nabla Q$), and the actor's motion and boundary work continuously reshape the field.
- **Individuation via Nonlinearity**: Through saturating nonlinear self-focusing, localized wave packets counteract dispersion to form stable **solitons**—autonomous, particle-like actors that emerge from and dissolve back into the continuum.

### 2. Hillman's Situated Image

James Hillman's distinction between *symbol* and *image* protects ecological particularity.[^1] A symbol reduces an appearance to a detached dictionary entry; an **image** retains its context, mood, topology, and internal relations.

In Dzog, an executable image is not a flat bitmap or a node-and-wire diagram. It is a situated whole:
- **Inhesion**: An actor's behavior depends on the impedance, damping, and wave activity of its surrounding neighborhood. Relocation into a stiffer or more viscous medium alters its resonance and pace automatically.
- **Relational Commitments**: Instead of scripting every frame of an animation, authors specify what yields, what reflects, what coheres, and which invariants hold under perturbation.

[^1]: The name *Dzog* nods to the contemplative concept of *lhundrub* (spontaneous presence) in Dzogchen: encountering a total situation prior to segmenting it into isolated conceptual categories. Here this serves strictly as an interface design analogy—letting users encounter and perturb a living field before demanding symbolic classification—not a metaphysical or contemplative claim.

---

## A Living Ecology of Actors

To make games and interactive applications feel genuinely alive, Dzog replaces rigid-body conventions with four ecological mechanisms:

1. **Radiative Presence**:
   Moving actors are never silent coordinates. A moving paddle or soliton continuously performs mechanical work on the lattice ($v_n = v_{\text{actor}}$), radiating Doppler-shifted pressure wakes and sensing returning echoes from the environment.
2. **Continuous Cohesion & Phase Transitions**:
   Structures possess a spatial cohesion field $C(\mathbf{x}) \in [0, 1]$ and material yield thresholds $p_{\text{yield}}(\mathbf{x})$. When struck—or when subjected to constructive wave interference from distant explosions—overpressure erodes local cohesion. Cracks open into semi-permeable waveguides that leak and refract energy before the structure cavitates back into fluid.
3. **Wave-Mediated Agency**:
   Actors influence one another across space through the medium. Constructive interference can cleave a weakened structure without direct contact; standing waves in a resonant cavity can trap, accelerate, or deflect passing trajectories.
4. **Heterogeneous Milieus**:
   Space is differentiated by mass density $\rho(\mathbf{x})$, bulk modulus $K(\mathbf{x})$, and dissipation $\gamma(\mathbf{x})$. Crystalline regions transmit fast, ringing high-frequency modes; dense basalt anvils reflect and channel shockwaves; viscous metamaterials lens and attenuate motion.

---

## The Communication Cycle

Authoring, playing, and debugging share a single experimental loop:

```text
Encounter -> Perturb -> Compare -> Commit -> Compose
```

1. **Encounter** an ongoing ecological situation directly through sight, sound, and motion.
2. **Perturb** it by injecting energy, moving a boundary, or painting material impedance.
3. **Compare** counterfactual branches by checkpointing the state, toggling a field coupling, and observing how trajectories diverge.
4. **Commit** relationships with explicit material bounds, yield thresholds, and boundary rules.
5. **Compose** field fragments into larger landscapes while inspecting how their boundary impedances couple.

A gesture is an utterance: injecting a wave alters dynamic state ($p, \mathbf{v}$); painting alters constitutive law ($\rho, K, \gamma, C$); switching view modes alters only perceptual projection.

---

## Multiple Perceptual Views

A single relational lattice supports multiple trustworthy projections:

- **Composite Experience View**: Renders the living scene—actors, wave compressions/rarefactions, kinetic shimmer, and structural stress fissures.
- **Bohmian & Pressure View ($p, Q$)**: Exposes the scalar wavefield alongside the quantum potential guidance ridges organizing trajectory flow.
- **Velocity & Energy Flux View ($\mathbf{v}, \mathbf{I} = p\mathbf{v}$)**: Reveals the instantaneous Poynting energy vector and hydrodynamic streaming.
- **Impedance & Cohesion View ($Z, C$)**: Inspects the underlying material commitments ($\sqrt{\rho K}$) and structural damage distribution.
- **Audible Projection**: Pressure probes sample localized lattice oscillations directly into audio streams, grounding visual and sonic feedback in the exact same evolving state.

Every semantic edit has an inspectable physical consequence; presentation changes never silently rewrite field laws.

---

## Precision & Engineering Boundaries

Using continuous field mechanics to power interactive worlds requires strict numerical discipline:

- **Discretization Honesty**: Staggered finite-difference lattices approximate a continuum within declared CFL and Nyquist limits. They do not eliminate dispersion, grid anisotropy, or GPU memory bandwidth costs.
- **Uniform Lattice Cost**: Dense stencil evaluation scales as $O(\text{cells} \times \text{substeps})$. Empty space still computes local field propagation—because in a field engine, "empty" space is an active medium carrying waves and potentials.
- **Hybrid Pragmatism**: The host runtime uses typed Rust structs, deterministic schedulers, and serialized RON manifests (`FieldSpec`, `Checkpoint`) to orchestrate GPU compute passes. Discrete identifiers are tools for inspection and composition, not failures of the field paradigm.

Uncertainty: low that field-coupled mechanics produce richer, more emergent interactive worlds than rigid-body scripts; medium on how far pure lattice PDEs scale to complex symbolic application logic without hybrid discrete-continuous coupling.

---

## References

- B. J. Hiley, [*Process, Distinction, Groupoids and Clifford Algebras*](https://arxiv.org/abs/1211.2107) — Process-first algebraic formulation of quantum mechanics and active information.
- D. Bohm & B. J. Hiley, *The Undivided Universe* — Ontological interpretation of quantum theory and the quantum potential $Q$.
- [Stanford Encyclopedia of Philosophy: Bohmian Mechanics](https://plato.stanford.edu/entries/qm-bohm/).
- James Hillman, [*An Inquiry into Image*](https://www.springpublications.com/springjournal/hillman_inquiry_into_image.html) — Particularity of the situated image versus detached symbolic labels.
