//! Dzog Core: Acoustic field PDE engine and reference solver.
//!
//! Implements linear acoustics on a heterogeneous 2D staggered finite-difference lattice:
//! - Center pressure p, East-face horizontal velocity vx, North-face vertical velocity vy.
//! - Conservative Yee stencil with time-staggered half-step velocity integration.
//! - Exponential dissipative losses: alpha = exp(-gamma * dt).
//! - Rigid walls, moving boundaries (paddles), and outer damping sponge.
//! - Hamiltonian energy accounting (kinetic, potential, total).
//! - Listener probes and offline audio export.
//! - Field specification and checkpoint persistence via RON.

pub mod audio;
pub mod body;
pub mod boundary;
pub mod energy;
pub mod excitation;
pub mod gravity;
pub mod grid;
pub mod manifest;
pub mod material;
pub mod probe;
pub mod solver;

pub use audio::{
    resample_probe_to_audio, write_stereo_wav, AudioExportConfig, BinauralCockpitSonifier,
    FieldAcousticFrame, FieldModalSynthesizer,
};
pub use body::{BodyMaterialSpec, CohesiveBody, MineralClass};
pub use boundary::{BoundaryMask, OuterBoundaryRule};
pub use energy::FieldEnergy;
pub use excitation::{
    AntiphaseShieldPulse, DirectionalWavePacket, GaussianImpulse, ReactionThrusterPlume,
};
pub use gravity::{GravityField, GravityWell};
pub use grid::{AcousticState, GridConfig};
pub use manifest::{Checkpoint, FieldSpec, ProbeDef};
pub use material::MaterialField;
pub use probe::{PressureProbe, ProbeSample};
pub use solver::{AcousticSolver, NonlinearLaw, SolverConfig};
