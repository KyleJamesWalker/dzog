//! Crystalline Corridor (`crates/shmup`): Bohm-Hiley Chladni Swarm & Annealing Shmup.

pub mod autopilot;
pub mod canyon;
pub mod entities;
pub mod field_game;
pub mod gui;

use std::time::Instant;
use field_game::ShmupFieldGame;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--headless" || a == "--bench") {
        run_headless_benchmark();
        return;
    }

    println!("Launching Dzog Crystalline Corridor (Shmup) GUI...");
    gui::launch_gui();
}

fn run_headless_benchmark() {
    println!("=== Dzog Crystalline Corridor (Shmup) Benchmark ===");
    let mut game = ShmupFieldGame::new(240, 150);
    game.autopilot_enabled = true;

    let t0 = Instant::now();
    for _ in 0..300 {
        game.step();
    }
    let elapsed = t0.elapsed();
    println!(
        "  Completed 300 macro-steps (1,200 FDTD substeps + Zero-Readback GPU Force Reductions) in {:.2?} (score={}, lobes={}/3, cubes_healed_player={}, cubes_healed_aliens={})",
        elapsed,
        game.score,
        game.player_intact_lobes(),
        game.cubes_collected_player,
        game.cubes_collected_aliens
    );
}
