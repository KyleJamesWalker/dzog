//! GPU-native acoustic field compute simulation pipeline.

use bytemuck::{Pod, Zeroable};
use dzog_core::*;
use wgpu::*;
use crate::context::GpuContext;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuSimParams {
    pub width: u32,
    pub height: u32,
    pub dx: f32,
    pub dy: f32,
    pub dt: f32,
    pub inv_dx: f32,
    pub inv_dy: f32,
    pub outer_rule: u32,
    pub beta: f32,
    pub alpha: f32,
    pub threshold_decay: f32,
    pub _pad: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuImpulseParams {
    pub width: u32,
    pub height: u32,
    pub center_x: f32,
    pub center_y: f32,
    pub two_sigma_sq: f32,
    pub amplitude: f32,
    pub dir_x: f32,
    pub dir_y: f32,
}

/// Compact GPU body/probe query descriptor (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuBodyQuery {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub yield_pressure: f32,
}

/// Compact GPU force, torque, overpressure, or point-probe reduction output (16 bytes).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct GpuBodyReduction {
    pub fx: f32,
    pub fy: f32,
    pub tau_z: f32,
    pub max_pressure: f32,
}

pub const MAX_GPU_BODY_QUERIES: usize = 128;

pub struct GpuSimulation {
    pub width: u32,
    pub height: u32,
    pub params: GpuSimParams,
    pub params_buffer: Buffer,
    pub impulse_buffer: Buffer,

    pub texture_a: Texture,
    pub view_a: TextureView,
    pub texture_b: Texture,
    pub view_b: TextureView,

    pub material_texture: Texture,
    pub material_view: TextureView,

    pub boundary_texture: Texture,
    pub boundary_view: TextureView,

    pub lum_texture_a: Texture,
    pub lum_view_a: TextureView,
    pub lum_texture_b: Texture,
    pub lum_view_b: TextureView,

    pub velocity_pipeline: ComputePipeline,
    pub pressure_pipeline: ComputePipeline,
    pub luminescence_pipeline: ComputePipeline,
    pub impulse_pipeline: ComputePipeline,
    pub reduce_pipeline: ComputePipeline,

    pub bind_group_velocity: BindGroup,
    pub bind_group_pressure: BindGroup,
    pub bind_group_luminescence: BindGroup,
    pub bind_group_impulse: BindGroup,
    pub bind_group_reduce: BindGroup,

    pub body_query_buffer: Buffer,
    pub body_reduction_buffer: Buffer,
    pub body_reduction_staging: Buffer,

    pub staging_buffer: Buffer,
    pub lum_staging_buffer: Buffer,
}

impl GpuSimulation {
    pub fn new(
        ctx: &GpuContext,
        state: &AcousticState,
        material: &MaterialField,
        boundary: &BoundaryMask,
        config: &SolverConfig,
    ) -> Result<Self, String> {
        let w = state.config.width as u32;
        let h = state.config.height as u32;

        let outer_rule = match boundary.outer_rule {
            OuterBoundaryRule::RigidWall => 0,
            OuterBoundaryRule::AbsorbingSponge => 1,
            OuterBoundaryRule::Periodic => 2,
        };

        let params = GpuSimParams {
            width: w,
            height: h,
            dx: state.config.dx,
            dy: state.config.dy,
            dt: config.dt,
            inv_dx: 1.0 / state.config.dx,
            inv_dy: 1.0 / state.config.dy,
            outer_rule,
            beta: config.nonlinearity.beta,
            alpha: config.nonlinearity.alpha,
            threshold_decay: config.nonlinearity.threshold_decay,
            _pad: 0.0,
        };

        let params_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Simulation Params Buffer"),
            size: std::mem::size_of::<GpuSimParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));

        let impulse_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Impulse Params Buffer"),
            size: std::mem::size_of::<GpuImpulseParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let texture_desc = TextureDescriptor {
            label: Some("State Ping-Pong Texture"),
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::STORAGE_BINDING
                | TextureUsages::COPY_SRC
                | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture_a = ctx.device.create_texture(&TextureDescriptor {
            label: Some("State Texture A"),
            ..texture_desc
        });
        let view_a = texture_a.create_view(&TextureViewDescriptor::default());

        let texture_b = ctx.device.create_texture(&TextureDescriptor {
            label: Some("State Texture B"),
            ..texture_desc
        });
        let view_b = texture_b.create_view(&TextureViewDescriptor::default());

        // Material Texture: (rho, K, gamma, 0)
        let material_texture = ctx.device.create_texture(&TextureDescriptor {
            label: Some("Material Texture"),
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            ..texture_desc
        });
        let material_view = material_texture.create_view(&TextureViewDescriptor::default());

        // Boundary Texture: (wall_east, wall_vx, wall_north, wall_vy)
        let boundary_texture = ctx.device.create_texture(&TextureDescriptor {
            label: Some("Boundary Texture"),
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            ..texture_desc
        });
        let boundary_view = boundary_texture.create_view(&TextureViewDescriptor::default());

        // Luminescence Ping-Pong Textures: (L, 0, 0, 0)
        let lum_texture_a = ctx.device.create_texture(&TextureDescriptor {
            label: Some("Luminescence Texture A"),
            ..texture_desc
        });
        let lum_view_a = lum_texture_a.create_view(&TextureViewDescriptor::default());

        let lum_texture_b = ctx.device.create_texture(&TextureDescriptor {
            label: Some("Luminescence Texture B"),
            ..texture_desc
        });
        let lum_view_b = lum_texture_b.create_view(&TextureViewDescriptor::default());

        // Shader Modules
        let vel_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Velocity Compute Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/velocity.wgsl").into()),
        });

        let press_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pressure Compute Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/pressure.wgsl").into()),
        });

        let lum_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Luminescence Compute Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/luminescence.wgsl").into()),
        });

        let impulse_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Impulse Compute Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/impulse.wgsl").into()),
        });

        let bind_group_layout = ctx.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Acoustic Compute BindGroupLayout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = ctx.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Acoustic Compute Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let velocity_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Velocity Pipeline"),
            layout: Some(&pipeline_layout),
            module: &vel_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        let pressure_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Pressure Pipeline"),
            layout: Some(&pipeline_layout),
            module: &press_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        let luminescence_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Luminescence Pipeline"),
            layout: Some(&pipeline_layout),
            module: &lum_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        let impulse_bind_group_layout = ctx.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Impulse Compute BindGroupLayout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let impulse_pipeline_layout = ctx.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Impulse Compute Pipeline Layout"),
            bind_group_layouts: &[&impulse_bind_group_layout],
            push_constant_ranges: &[],
        });

        let impulse_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Impulse Pipeline"),
            layout: Some(&impulse_pipeline_layout),
            module: &impulse_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        // Pass 1 BindGroup: Read A -> Write B
        let bind_group_velocity = ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Velocity BindGroup (A -> B)"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&view_a) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&material_view) },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&boundary_view) },
                BindGroupEntry { binding: 4, resource: BindingResource::TextureView(&view_b) },
            ],
        });

        // Pass 2 BindGroup: Read B -> Write A
        let bind_group_pressure = ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Pressure BindGroup (B -> A)"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&view_b) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&material_view) },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&boundary_view) },
                BindGroupEntry { binding: 4, resource: BindingResource::TextureView(&view_a) },
            ],
        });

        // Pass 3 BindGroup: Read State A + Material + Lum A -> Write Lum B
        let bind_group_luminescence = ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Luminescence BindGroup (Lum A -> Lum B)"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&view_a) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&material_view) },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&lum_view_a) },
                BindGroupEntry { binding: 4, resource: BindingResource::TextureView(&lum_view_b) },
            ],
        });

        // Impulse BindGroup: Read A -> Write B (followed by copy B -> A)
        let bind_group_impulse = ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Impulse BindGroup (A -> B)"),
            layout: &impulse_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: impulse_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&view_a) },
                BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&material_view) },
                BindGroupEntry { binding: 3, resource: BindingResource::TextureView(&view_b) },
            ],
        });

        // Zero-readback GPU Body Force & Probe Reduction pipeline
        let reduce_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Body Force & Probe Reduction Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/reduce_body.wgsl").into()),
        });

        let body_buf_bytes = (MAX_GPU_BODY_QUERIES * std::mem::size_of::<GpuBodyQuery>()) as u64;
        let body_query_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Body Query Storage Buffer"),
            size: body_buf_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let body_reduction_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Body Reduction Storage Buffer"),
            size: body_buf_bytes,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let body_reduction_staging = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Body Reduction Staging Buffer"),
            size: body_buf_bytes,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let reduce_bind_group_layout = ctx.device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Body Reduction BindGroupLayout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let reduce_pipeline_layout = ctx.device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Body Reduction Pipeline Layout"),
            bind_group_layouts: &[&reduce_bind_group_layout],
            push_constant_ranges: &[],
        });

        let reduce_pipeline = ctx.device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Body Reduction Pipeline"),
            layout: Some(&reduce_pipeline_layout),
            module: &reduce_shader,
            entry_point: Some("main"),
            compilation_options: PipelineCompilationOptions::default(),
            cache: None,
        });

        let bind_group_reduce = ctx.device.create_bind_group(&BindGroupDescriptor {
            label: Some("Body Reduction BindGroup"),
            layout: &reduce_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: BindingResource::TextureView(&view_a) },
                BindGroupEntry { binding: 2, resource: body_query_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: body_reduction_buffer.as_entire_binding() },
            ],
        });

        // Staging buffers for readback
        let bytes_per_row = (w * 16 + 255) & !255; // WGPU row alignment: 256 bytes
        let staging_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Simulation Staging Readback Buffer"),
            size: (bytes_per_row * h) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let lum_staging_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Luminescence Staging Readback Buffer"),
            size: (bytes_per_row * h) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let sim = Self {
            width: w,
            height: h,
            params,
            params_buffer,
            impulse_buffer,
            texture_a,
            view_a,
            texture_b,
            view_b,
            material_texture,
            material_view,
            boundary_texture,
            boundary_view,
            lum_texture_a,
            lum_view_a,
            lum_texture_b,
            lum_view_b,
            velocity_pipeline,
            pressure_pipeline,
            luminescence_pipeline,
            impulse_pipeline,
            reduce_pipeline,
            bind_group_velocity,
            bind_group_pressure,
            bind_group_luminescence,
            bind_group_impulse,
            bind_group_reduce,
            body_query_buffer,
            body_reduction_buffer,
            body_reduction_staging,
            staging_buffer,
            lum_staging_buffer,
        };

        sim.upload_state(ctx, state);
        sim.upload_material(ctx, material);
        sim.upload_boundary(ctx, boundary);

        Ok(sim)
    }

    /// Upload CPU state to GPU Texture A and Luminescence Texture A.
    pub fn upload_state(&self, ctx: &GpuContext, state: &AcousticState) {
        let count = state.config.cell_count();
        let has_lum = state.luminescence.len() == count;
        let mut buffer = Vec::with_capacity(count * 4);
        let mut lum_buf = Vec::with_capacity(count * 4);
        for i in 0..count {
            buffer.push(state.pressure[i]);
            buffer.push(state.vx[i]);
            buffer.push(state.vy[i]);
            buffer.push(0.0f32);

            lum_buf.push(if has_lum { state.luminescence[i] } else { 0.0f32 });
            lum_buf.push(0.0f32);
            lum_buf.push(0.0f32);
            lum_buf.push(0.0f32);
        }

        let extent = Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
        let layout = TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(self.width * 16),
            rows_per_image: Some(self.height),
        };

        ctx.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&buffer),
            layout,
            extent,
        );

        ctx.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.lum_texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&lum_buf),
            layout,
            extent,
        );
    }

    /// Upload material parameters to GPU Material Texture.
    pub fn upload_material(&self, ctx: &GpuContext, material: &MaterialField) {
        let count = material.config.cell_count();
        let has_cohesion = material.cohesion.len() == count;
        let mut buffer = Vec::with_capacity(count * 4);
        for i in 0..count {
            buffer.push(material.density[i]);
            buffer.push(material.bulk_modulus[i]);
            buffer.push(material.damping[i]);
            buffer.push(if has_cohesion { material.cohesion[i] } else { 0.0f32 });
        }

        ctx.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.material_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&buffer),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 16),
                rows_per_image: Some(self.height),
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Inject a Gaussian or Directional wave packet directly on the GPU without CPU readback.
    #[allow(clippy::too_many_arguments)]
    pub fn inject_impulse_gpu(
        &self,
        ctx: &GpuContext,
        center_x: f32,
        center_y: f32,
        sigma: f32,
        amplitude: f32,
        dir_x: f32,
        dir_y: f32,
    ) {
        let impulse_params = GpuImpulseParams {
            width: self.width,
            height: self.height,
            center_x,
            center_y,
            two_sigma_sq: 2.0 * sigma * sigma,
            amplitude,
            dir_x,
            dir_y,
        };
        ctx.queue
            .write_buffer(&self.impulse_buffer, 0, bytemuck::bytes_of(&impulse_params));

        let workgroups_x = (self.width + 15) / 16;
        let workgroups_y = (self.height + 15) / 16;

        let mut encoder = ctx.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("GPU Impulse Excitation Encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Impulse Pass (A -> B)"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.impulse_pipeline);
            pass.set_bind_group(0, &self.bind_group_impulse, &[]);
            pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        encoder.copy_texture_to_texture(
            TexelCopyTextureInfo {
                texture: &self.texture_b,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyTextureInfo {
                texture: &self.texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Upload boundary wall masks and velocities to GPU Boundary Texture.
    pub fn upload_boundary(&self, ctx: &GpuContext, boundary: &BoundaryMask) {
        let count = boundary.config.cell_count();
        let mut buffer = Vec::with_capacity(count * 4);
        for i in 0..count {
            let we = if boundary.wall_east[i] { 1.0f32 } else { 0.0f32 };
            let wvx = boundary.wall_vx[i];
            let wn = if boundary.wall_north[i] { 1.0f32 } else { 0.0f32 };
            let wvy = boundary.wall_vy[i];
            buffer.push(we);
            buffer.push(wvx);
            buffer.push(wn);
            buffer.push(wvy);
        }

        ctx.queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.boundary_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&buffer),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 16),
                rows_per_image: Some(self.height),
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Dispatch one or more simulation substeps on the GPU.
    pub fn step(&self, ctx: &GpuContext, substeps: usize) {
        let workgroups_x = (self.width + 15) / 16;
        let workgroups_y = (self.height + 15) / 16;

        let mut encoder = ctx.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Simulation Compute Substeps Encoder"),
        });

        for _ in 0..substeps {
            // Pass 1: Velocity update (A -> B)
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Velocity Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.velocity_pipeline);
                pass.set_bind_group(0, &self.bind_group_velocity, &[]);
                pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }

            // Pass 2: Pressure update (B -> A)
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Pressure Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pressure_pipeline);
                pass.set_bind_group(0, &self.bind_group_pressure, &[]);
                pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }

            // Pass 3: Screened reaction-diffusion luminescence update (Lum A -> Lum B -> Lum A)
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Luminescence Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.luminescence_pipeline);
                pass.set_bind_group(0, &self.bind_group_luminescence, &[]);
                pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
            }

            encoder.copy_texture_to_texture(
                TexelCopyTextureInfo {
                    texture: &self.lum_texture_b,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyTextureInfo {
                    texture: &self.lum_texture_a,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Read back simulation state from GPU Texture A & Luminescence Texture A into an existing `AcousticState`.
    pub fn readback_state_into(&self, ctx: &GpuContext, state: &mut AcousticState) {
        let bytes_per_row = (self.width * 16 + 255) & !255;
        let mut encoder = ctx.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Readback Encoder"),
        });

        let extent = Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: 1,
        };
        let layout = TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row),
            rows_per_image: Some(self.height),
        };

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &self.staging_buffer,
                layout,
            },
            extent,
        );

        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: &self.lum_texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &self.lum_staging_buffer,
                layout,
            },
            extent,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));

        let slice = self.staging_buffer.slice(..);
        let lum_slice = self.lum_staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        let tx2 = tx.clone();
        slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        lum_slice.map_async(MapMode::Read, move |result| {
            tx2.send(result).unwrap();
        });

        ctx.device.poll(Maintain::Wait);
        rx.recv().unwrap().unwrap();
        rx.recv().unwrap().unwrap();

        let grid = state.config;
        let count = grid.cell_count();
        if state.luminescence.len() != count {
            state.luminescence.resize(count, 0.0);
        }

        {
            let view = slice.get_mapped_range();
            let lum_view = lum_slice.get_mapped_range();
            let row_bytes = (self.width * 16) as usize;
            let pitch = bytes_per_row as usize;

            for y in 0..self.height as usize {
                let row_start = y * pitch;
                let row_slice = &view[row_start..row_start + row_bytes];
                let lum_row_slice = &lum_view[row_start..row_start + row_bytes];
                let floats: &[f32] = bytemuck::cast_slice(row_slice);
                let lum_floats: &[f32] = bytemuck::cast_slice(lum_row_slice);

                for x in 0..self.width as usize {
                    let idx = grid.index(x, y);
                    let f_idx = x * 4;
                    state.pressure[idx] = floats[f_idx];
                    state.vx[idx] = floats[f_idx + 1];
                    state.vy[idx] = floats[f_idx + 2];
                    state.luminescence[idx] = lum_floats[f_idx];
                }
            }
        }

        self.staging_buffer.unmap();
        self.lum_staging_buffer.unmap();
    }

    /// Read back simulation state from GPU Texture A to CPU AcousticState.
    pub fn readback_state(&self, ctx: &GpuContext) -> AcousticState {
        let grid = GridConfig::new(
            self.width as usize,
            self.height as usize,
            self.params.dx,
            self.params.dy,
        );
        let mut state = AcousticState::new(grid);
        self.readback_state_into(ctx, &mut state);
        state
    }

    /// Update the outer boundary rule dynamically (e.g. toggling Periodic vs AbsorbingSponge).
    pub fn set_outer_rule(&mut self, ctx: &GpuContext, outer_rule: OuterBoundaryRule) {
        self.params.outer_rule = match outer_rule {
            OuterBoundaryRule::RigidWall => 0,
            OuterBoundaryRule::AbsorbingSponge => 1,
            OuterBoundaryRule::Periodic => 2,
        };
        ctx.queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.params));
    }

    /// Execute a zero-readback GPU Force, Torque, Fracture & Probe Reduction Pass.
    ///
    /// Evaluates net field forces `(fx, fy, tau_z)` and peak overpressure `max_pressure`
    /// for up to `MAX_GPU_BODY_QUERIES` bodies/probes directly on the GPU, reading back
    /// only the compact `16 * N` byte reduction array instead of the full field texture.
    pub fn reduce_bodies_gpu(
        &self,
        ctx: &GpuContext,
        queries: &[GpuBodyQuery],
    ) -> Vec<GpuBodyReduction> {
        let count = queries.len().min(MAX_GPU_BODY_QUERIES);
        if count == 0 {
            return Vec::new();
        }

        let mut padded = [GpuBodyQuery {
            x: 0.0,
            y: 0.0,
            radius: 0.0,
            yield_pressure: 0.0,
        }; MAX_GPU_BODY_QUERIES];
        padded[..count].copy_from_slice(&queries[..count]);

        ctx.queue
            .write_buffer(&self.body_query_buffer, 0, bytemuck::bytes_of(&padded));

        let byte_len = (count * std::mem::size_of::<GpuBodyReduction>()) as u64;
        let mut encoder = ctx.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Body Reduction Encoder"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Body Force & Probe Reduction Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.reduce_pipeline);
            pass.set_bind_group(0, &self.bind_group_reduce, &[]);
            pass.dispatch_workgroups(((count as u32) + 31) / 32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            &self.body_reduction_buffer,
            0,
            &self.body_reduction_staging,
            0,
            byte_len,
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));

        let slice = self.body_reduction_staging.slice(0..byte_len);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |res| {
            tx.send(res).unwrap();
        });
        ctx.device.poll(Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let out = {
            let view = slice.get_mapped_range();
            let items: &[GpuBodyReduction] = bytemuck::cast_slice(&view);
            items.to_vec()
        };
        self.body_reduction_staging.unmap();
        out
    }
}
