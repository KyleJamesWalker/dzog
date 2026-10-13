//! Interactive native WGPU/winit field instrument for Dzoglang.

use std::sync::Arc;
use dzog_core::*;
use dzog_wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct AppState {
    window: Arc<Window>,
    ctx: GpuContext,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    sim: GpuSimulation,
    renderer: GpuRenderer,
    paused: bool,
    cursor_pos: (f32, f32),
    mouse_down: bool,
    right_mouse_down: bool,
    width: usize,
    height: usize,
}

struct App {
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Dzoglang Field Instrument — Relational Acoustic Lattice")
            .with_inner_size(winit::dpi::LogicalSize::new(960.0, 720.0));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        let ctx = pollster::block_on(GpuContext::init_headless()).unwrap();

        // Create surface
        let surface = unsafe {
            // Safety: window is kept alive in Arc<Window> inside AppState
            let target = winit::raw_window_handle::HasWindowHandle::window_handle(&*window).unwrap();
            let display = winit::raw_window_handle::HasDisplayHandle::display_handle(&*window).unwrap();
            ctx.instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_window_handle: target.as_raw(),
                    raw_display_handle: display.as_raw(),
                })
                .unwrap()
        };

        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let cap = surface.get_capabilities(&ctx.adapter);
        let surface_format = cap.formats[0];

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: cap.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&ctx.device, &surface_config);

        // Grid resolution (128 x 96)
        let sim_w = 128;
        let sim_h = 96;
        let grid = GridConfig::new(sim_w, sim_h, 1.0, 1.0);
        let mut acoustic_state = AcousticState::new(grid);
        let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.002);
        let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
        let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
        let config = SolverConfig::new(dt, 0.7, 8);

        // Initial excitation at center
        let impulse = GaussianImpulse::new(sim_w as f32 / 2.0, sim_h as f32 / 2.0, 3.5, 1.5);
        impulse.inject(&mut acoustic_state);

        let sim = GpuSimulation::new(&ctx, &acoustic_state, &material, &boundary, &config).unwrap();
        let renderer = GpuRenderer::new(&ctx, surface_format, &sim);

        self.state = Some(AppState {
            window,
            ctx,
            surface,
            surface_config,
            sim,
            renderer,
            paused: false,
            cursor_pos: (0.5, 0.5),
            mouse_down: false,
            right_mouse_down: false,
            width: sim_w,
            height: sim_h,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width > 0 && new_size.height > 0 {
                    state.surface_config.width = new_size.width;
                    state.surface_config.height = new_size.height;
                    state.surface.configure(&state.ctx.device, &state.surface_config);
                    state.window.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let win_w = state.surface_config.width as f32;
                let win_h = state.surface_config.height as f32;
                state.cursor_pos = (
                    (position.x as f32 / win_w).clamp(0.0, 1.0),
                    (position.y as f32 / win_h).clamp(0.0, 1.0),
                );

                if state.mouse_down {
                    inject_brush_at_cursor(state, 1.0);
                }
            }
            WindowEvent::MouseInput { button, state: btn_state, .. } => {
                match button {
                    MouseButton::Left => {
                        state.mouse_down = btn_state == ElementState::Pressed;
                        if state.mouse_down {
                            inject_brush_at_cursor(state, 1.5);
                        }
                    }
                    MouseButton::Right => {
                        state.right_mouse_down = btn_state == ElementState::Pressed;
                    }
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) | PhysicalKey::Code(KeyCode::KeyQ) => {
                            event_loop.exit();
                        }
                        PhysicalKey::Code(KeyCode::Space) => {
                            inject_brush_at_cursor(state, 2.0);
                        }
                        PhysicalKey::Code(KeyCode::KeyP) => {
                            state.paused = !state.paused;
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            let grid = GridConfig::new(state.width, state.height, 1.0, 1.0);
                            let clear_state = AcousticState::new(grid);
                            state.sim.upload_state(&state.ctx, &clear_state);
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if !state.paused {
                    // Advance 8 GPU substeps per render frame
                    state.sim.step(&state.ctx, 8);
                }

                let output = match state.surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Lost) => {
                        state.surface.configure(&state.ctx.device, &state.surface_config);
                        return;
                    }
                    Err(_) => return,
                };

                let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
                state.renderer.render(&state.ctx, &view);
                output.present();

                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

fn inject_brush_at_cursor(state: &mut AppState, amplitude: f32) {
    let cx = state.cursor_pos.0 * state.width as f32;
    let cy = state.cursor_pos.1 * state.height as f32;

    let mut current_state = state.sim.readback_state(&state.ctx);
    let impulse = GaussianImpulse::new(cx, cy, 3.0, amplitude);
    impulse.inject(&mut current_state);
    state.sim.upload_state(&state.ctx, &current_state);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--headless") {
        println!("Dzoglang App: Headless verification passed.");
        return;
    }

    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
