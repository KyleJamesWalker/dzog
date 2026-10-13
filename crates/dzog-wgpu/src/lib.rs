//! Dzoglang WGPU runtime, compute pipelines, and shaders.

pub mod context;
pub mod renderer;
pub mod simulation;

pub use context::GpuContext;
pub use renderer::GpuRenderer;
pub use simulation::{
    GpuBodyQuery, GpuBodyReduction, GpuImpulseParams, GpuSimParams, GpuSimulation,
    MAX_GPU_BODY_QUERIES,
};
