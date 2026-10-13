# Agent Instructions for Dzog

## Mandatory Pre-Reading

Before planning, editing code, or modifying documentation in this repository, **you must read**:

1. [`philosophy.md`](./philosophy.md) — Core thesis: relation-first computing, Bohm-Hiley active information ($Q = -\nabla^2 R / R$), Hillman's situated image, and a living ecology of field-coupled actors.
2. [`readme.md`](./readme.md) — Governing PDEs, Yee-staggered discretization, workspace crate architecture (`dzog-core`, `dzog-wgpu`, `dzog-bevy`, `pong`, `asteroids`), and verification commands.

## Architectural & Design Principles

1. **Field-First Causality**:
   - Do not introduce classical rigid-body engines, isolated OOP state scripts, or cosmetic particle systems detached from the lattice.
   - Actors (solitons, paddles, breakable structures, resonators) must couple bidirectionally to the `dzog-core` staggered field ($p, \mathbf{v}, \rho, K, \gamma, C$).
2. **Living Ecology of Actors**:
   - Entities should radiate into, sense, and reshape their shared milieu through impedance contrasts ($Z = \sqrt{\rho K}$), continuous cohesion erosion ($C(\mathbf{x}) \in [0, 1]$), radiation pressure ($-\nabla p$), and Bohmian quantum potential guidance ($-\nabla Q$).
3. **Numerical Honesty & Stability**:
   - Preserve the conservative staggered Yee stencil (`p` at cell centers, `vx` on East faces, `vy` on North faces) and exponential damping (`exp(-gamma * dt)`).
   - Never violate the global CFL bound (`SolverConfig::compute_cfl_dt`). Keep `dt` fixed during a run.
   - Keep CPU (`dzog-core`) and GPU (`dzog-wgpu`) stencils numerically aligned within tested tolerances ($< 10^{-4}$).
4. **Delegate Host Capabilities to Bevy**:
   - Do not duplicate windowing, input, ECS, asset, or audio output/sink infrastructure that Bevy already provides (e.g., use `bevy_audio`'s `Decodable` / `AudioPlayer` / `AudioSink` rather than bespoke `cpal` device threads). Keep `dzog-core` focused strictly on PDE field physics, wave-matter coupling, and pure sample math.
5. **Verification Gate**:
   - Run `cargo test --workspace` after changes to verify analytical propagation, impedance reflection, energy conservation, soliton cohesion, and gameplay physics.
