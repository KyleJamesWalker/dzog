//! Dzog WGPU compute runtime, pipelines, and WGSL shaders.

pub mod context;
pub mod reduce;
pub mod simulation;

pub use context::GpuContext;
pub use reduce::reduce_bodies_cpu;
pub use simulation::{
    GpuBodyQuery, GpuBodyReduction, GpuImpulseParams, GpuSimParams, GpuSimulation,
    MAX_GPU_BODY_QUERIES,
};
