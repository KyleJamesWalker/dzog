//! WGPU device and queue setup supporting headless compute and windowed targets.

use std::sync::OnceLock;
use wgpu::*;

#[derive(Clone)]
pub struct GpuContext {
    pub instance: Instance,
    pub adapter: Adapter,
    pub device: Device,
    pub queue: Queue,
}

static SHARED_GPU_CONTEXT: OnceLock<GpuContext> = OnceLock::new();

impl GpuContext {
    /// Get or initialize the shared hardware WGPU context.
    pub fn shared() -> &'static Self {
        SHARED_GPU_CONTEXT.get_or_init(|| {
            pollster::block_on(Self::init_headless_fresh())
                .expect("Dzog requires a WGPU-capable GPU adapter")
        })
    }

    /// Initialize headless GPU context (reuses shared device handle).
    pub async fn init_headless() -> Result<Self, String> {
        if let Some(ctx) = SHARED_GPU_CONTEXT.get() {
            return Ok(ctx.clone());
        }
        let ctx = Self::init_headless_fresh().await?;
        let _ = SHARED_GPU_CONTEXT.set(ctx.clone());
        Ok(ctx)
    }

    async fn init_headless_fresh() -> Result<Self, String> {
        let instance = Instance::new(&InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| "Failed to find suitable WGPU GPU adapter".to_string())?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    label: Some("Dzog Headless Device"),
                    required_features: Features::empty(),
                    required_limits: Limits::default(),
                    memory_hints: MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to request WGPU device: {e}"))?;

        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}
