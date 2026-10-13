//! Dzoglang Field-First Solid-State Breakout Benchmark & GUI.

pub mod field_game;
pub mod gui;

use std::time::Instant;
use dzog_wgpu::{GpuContext, GpuSimulation};
use field_game::BreakoutFieldGame;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--headless" || a == "--bench") {
        run_headless_benchmark();
        return;
    }

    println!("Launching Dzoglang Solid-State & Quantum-Vacuum Breakout GUI...");
    gui::launch_gui();
}

fn run_headless_benchmark() {
    println!("=== Dzoglang Solid-State & Quantum-Vacuum Breakout Benchmark ===");
    let mut game = BreakoutFieldGame::new(128, 80);

    // 1. WGPU Hardware Compute Benchmark
    if let Ok(ctx) = pollster::block_on(GpuContext::init_headless()) {
        println!("[1/3] Benchmarking WGPU Hardware Compute Pipeline (128x80 grid, 4,000 FDTD substeps)...");
        let gpu_sim = GpuSimulation::new(
            &ctx,
            &game.solver.state,
            &game.solver.material,
            &game.solver.boundary,
            &game.solver.config,
        )
        .expect("Failed to initialize WGPU simulation");

        let t_gpu = Instant::now();
        gpu_sim.inject_impulse_gpu(&ctx, 64.0, 40.0, 2.5, 3.0, 0.0, -1.0);
        gpu_sim.step(&ctx, 4000);
        let _state = gpu_sim.readback_state(&ctx);
        let gpu_elapsed = t_gpu.elapsed();
        println!(
            "  WGPU completed 4,000 FDTD substeps + readback in {:.2?}",
            gpu_elapsed
        );
    }

    // 2. CPU Reference Solver + Breakout Mechanics Benchmark
    println!("[2/3] Benchmarking Breakout Lattice Stepping (128x80 grid, 32 crystal bricks)...");
    let t0 = Instant::now();
    game.serve();

    for _ in 0..1000 {
        game.step();
    }
    let elapsed = t0.elapsed();
    println!(
        "  Completed 1,000 macro-steps (4,000 FDTD substeps) in {:.2?}",
        elapsed
    );

    // 3. Pixel Framebuffer Rasterization
    println!("[3/3] Benchmarking Pixel Framebuffer Rasterization (10,240 pixels)...");
    let t1 = Instant::now();
    for _ in 0..1000 {
        game.render_to_pixel_buffer();
    }
    let raster_elapsed = t1.elapsed();
    println!("  Rasterized 1,000 frames in {:.2?}", raster_elapsed);

    println!("Benchmark finished successfully.");
}
