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
}

pub struct GpuSimulation {
    pub width: u32,
    pub height: u32,
    pub params: GpuSimParams,
    pub params_buffer: Buffer,

    pub texture_a: Texture,
    pub view_a: TextureView,
    pub texture_b: Texture,
    pub view_b: TextureView,

    pub material_texture: Texture,
    pub material_view: TextureView,

    pub boundary_texture: Texture,
    pub boundary_view: TextureView,

    pub velocity_pipeline: ComputePipeline,
    pub pressure_pipeline: ComputePipeline,

    pub bind_group_velocity: BindGroup,
    pub bind_group_pressure: BindGroup,

    pub staging_buffer: Buffer,
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
        };

        let params_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Simulation Params Buffer"),
            size: std::mem::size_of::<GpuSimParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));

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

        // Shader Modules
        let vel_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Velocity Compute Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/velocity.wgsl").into()),
        });

        let press_shader = ctx.device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Pressure Compute Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/pressure.wgsl").into()),
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

        // Staging buffer for readback
        let bytes_per_row = (w * 16 + 255) & !255; // WGPU row alignment: 256 bytes
        let staging_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("Simulation Staging Readback Buffer"),
            size: (bytes_per_row * h) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let sim = Self {
            width: w,
            height: h,
            params,
            params_buffer,
            texture_a,
            view_a,
            texture_b,
            view_b,
            material_texture,
            material_view,
            boundary_texture,
            boundary_view,
            velocity_pipeline,
            pressure_pipeline,
            bind_group_velocity,
            bind_group_pressure,
            staging_buffer,
        };

        sim.upload_state(ctx, state);
        sim.upload_material(ctx, material);
        sim.upload_boundary(ctx, boundary);

        Ok(sim)
    }

    /// Upload CPU state to GPU Texture A.
    pub fn upload_state(&self, ctx: &GpuContext, state: &AcousticState) {
        let count = state.config.cell_count();
        let mut buffer = Vec::with_capacity(count * 4);
        for i in 0..count {
            buffer.push(state.pressure[i]);
            buffer.push(state.vx[i]);
            buffer.push(state.vy[i]);
            buffer.push(0.0f32);
        }

        ctx.queue.write_texture(
            ImageCopyTexture {
                texture: &self.texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&buffer),
            ImageDataLayout {
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

    /// Upload material parameters to GPU Material Texture.
    pub fn upload_material(&self, ctx: &GpuContext, material: &MaterialField) {
        let count = material.config.cell_count();
        let mut buffer = Vec::with_capacity(count * 4);
        for i in 0..count {
            buffer.push(material.density[i]);
            buffer.push(material.bulk_modulus[i]);
            buffer.push(material.damping[i]);
            buffer.push(0.0f32);
        }

        ctx.queue.write_texture(
            ImageCopyTexture {
                texture: &self.material_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&buffer),
            ImageDataLayout {
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
            ImageCopyTexture {
                texture: &self.boundary_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            bytemuck::cast_slice(&buffer),
            ImageDataLayout {
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
        }

        ctx.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Read back simulation state from GPU Texture A to CPU AcousticState.
    pub fn readback_state(&self, ctx: &GpuContext) -> AcousticState {
        let bytes_per_row = (self.width * 16 + 255) & !255;
        let mut encoder = ctx.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Readback Encoder"),
        });

        encoder.copy_texture_to_buffer(
            ImageCopyTexture {
                texture: &self.texture_a,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            ImageCopyBuffer {
                buffer: &self.staging_buffer,
                layout: ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        ctx.queue.submit(std::iter::once(encoder.finish()));

        // Map buffer and read floats
        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        ctx.device.poll(Maintain::Wait);
        rx.recv().unwrap().unwrap();

        let grid = GridConfig::new(
            self.width as usize,
            self.height as usize,
            self.params.dx,
            self.params.dy,
        );
        let mut state = AcousticState::new(grid);

        {
            let view = slice.get_mapped_range();
            let row_bytes = (self.width * 16) as usize;
            let pitch = bytes_per_row as usize;

            for y in 0..self.height as usize {
                let row_start = y * pitch;
                let row_slice = &view[row_start..row_start + row_bytes];
                let floats: &[f32] = bytemuck::cast_slice(row_slice);

                for x in 0..self.width as usize {
                    let idx = grid.index(x, y);
                    let f_idx = x * 4;
                    state.pressure[idx] = floats[f_idx];
                    state.vx[idx] = floats[f_idx + 1];
                    state.vy[idx] = floats[f_idx + 2];
                }
            }
        }

        self.staging_buffer.unmap();
        state
    }
}
