//! Dzog WGPU compute runtime, pipelines, and WGSL shaders.

pub mod context;
pub mod simulation;

pub use context::GpuContext;
pub use simulation::{
    GpuBodyQuery, GpuBodyReduction, GpuImpulseParams, GpuSimParams, GpuSimulation,
    MAX_GPU_BODY_QUERIES,
};
