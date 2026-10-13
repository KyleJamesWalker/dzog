//! Dzog Field-First Crystalline Asteroids & Alien Ecology Demo & Benchmark.

pub mod field_game;
pub mod gui;

use std::time::Instant;
use dzog_wgpu::{GpuBodyQuery, GpuContext, GpuSimulation};
use field_game::AsteroidsFieldGame;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--headless" || a == "--bench") {
        run_headless_benchmark();
        return;
    }

    println!("Launching Dzog Field-First Crystalline Asteroids & Alien Ecology GUI...");
    gui::launch_gui();
}

fn run_headless_benchmark() {
    println!("=== Dzog Field-First Crystalline Asteroids & Alien Ecology Benchmark ===");
    let mut game = AsteroidsFieldGame::new(144, 96);

    if let Ok(ctx) = pollster::block_on(GpuContext::init_headless()) {
        println!(
            "[1/3] Benchmarking Zero-Readback WGPU Compute + Body Force Reduction (144x96 periodic grid)..."
        );
        let gpu_sim = GpuSimulation::new(
            &ctx,
            &game.solver.state,
            &game.solver.material,
            &game.solver.boundary,
            &game.solver.config,
        )
        .expect("Failed to initialize WGPU simulation");

        let queries: Vec<GpuBodyQuery> = game
            .asteroids
            .iter()
            .map(|r| GpuBodyQuery {
                x: r.x,
                y: r.y,
                radius: r.radius,
                yield_pressure: r.material.yield_pressure,
            })
            .collect();

        let t_gpu = Instant::now();
        gpu_sim.inject_impulse_gpu(&ctx, 72.0, 48.0, 2.5, 3.0, 1.0, 0.0);
        for _ in 0..250 {
            gpu_sim.step(&ctx, 4);
            let _reds = gpu_sim.reduce_bodies_gpu(&ctx, &queries);
        }
        let gpu_elapsed = t_gpu.elapsed();
        println!(
            "  Completed 1,000 FDTD substeps + 250 Zero-Readback GPU Force Reductions in {:.2?}",
            gpu_elapsed
        );
    }

    println!(
        "[2/3] Benchmarking Rotating Cohesive Bodies, Alien Bohmian AI & Toroidal PDE Stepping..."
    );
    let t0 = Instant::now();
    for step in 0..400 {
        if step % 25 == 0 {
            game.thrust_forward(1.0);
            game.fire_soliton_bolt(step % 50 == 0);
        }
        if step % 60 == 30 {
            game.trigger_antiphase_shield();
        }
        game.step();
    }
    let elapsed = t0.elapsed();
    println!(
        "  Completed 400 macro-steps (1,600 FDTD substeps) in {:.2?} (score={}, cleaves={})",
        elapsed, game.score, game.topological_cleaves
    );

    println!("[3/3] Benchmarking Multi-View Framebuffer Rasterization...");
    let t1 = Instant::now();
    for _ in 0..400 {
        game.render_to_pixel_buffer();
    }
    println!("  Rasterized 400 frames in {:.2?}", t1.elapsed());
}
