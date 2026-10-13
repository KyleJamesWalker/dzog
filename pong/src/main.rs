//! Dzoglang Field-First Breakout Pong Benchmark & GUI.

pub mod field_game;
pub mod gui;

use std::time::Instant;
use field_game::BreakoutFieldGame;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--headless" || a == "--bench") {
        run_headless_benchmark();
        return;
    }

    // Launch Bevy GUI by default
    println!("Launching Dzoglang Soliton Breakout GUI...");
    gui::launch_gui();
}

fn run_headless_benchmark() {
    println!("=== Dzoglang Field-First Breakout Pong Benchmark ===");
    let mut game = BreakoutFieldGame::new(128, 80);

    println!("[1/2] Benchmarking Breakout Lattice Stepping (128x80 grid, 32 bricks)...");
    let t0 = Instant::now();
    game.serve();

    for _ in 0..1000 {
        game.step();
    }
    let elapsed = t0.elapsed();
    println!("  Completed 1,000 macro-steps (4,000 FDTD substeps) in {:.2?}", elapsed);

    println!("[2/2] Benchmarking Pixel Framebuffer Rasterization (10,240 pixels)...");
    let t1 = Instant::now();
    for _ in 0..1000 {
        game.render_to_pixel_buffer();
    }
    let raster_elapsed = t1.elapsed();
    println!("  Rasterized 1,000 frames in {:.2?}", raster_elapsed);

    println!("Benchmark finished successfully.");
}
