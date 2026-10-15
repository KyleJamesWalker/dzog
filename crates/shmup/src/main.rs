//! Crystalline Corridor (`crates/shmup`): Bohm-Hiley Chladni Swarm & Annealing Shmup.

pub mod autopilot;
pub mod canyon;
pub mod combat;
pub mod entities;
pub mod field_game;
pub mod gui;
pub mod render;
pub mod spawning;

use std::time::Instant;
use field_game::ShmupFieldGame;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if let Some(pos) = args.iter().position(|a| a == "--isolate") {
        let out_dir = args
            .get(pos + 1)
            .cloned()
            .unwrap_or_else(|| "/tmp/opencode".to_string());
        capture_isolated_toy_scenarios(&out_dir);
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--screenshot-wreckage") {
        let out_path = args
            .get(pos + 1)
            .cloned()
            .unwrap_or_else(|| "docs/shmup_wreckage_verify.png".to_string());
        capture_wreckage_test_screenshot(&out_path);
        return;
    }

    if let Some(pos) = args.iter().position(|a| a == "--screenshot") {
        let out_path = args
            .get(pos + 1)
            .cloned()
            .unwrap_or_else(|| "shmup_screenshot.png".to_string());
        capture_headless_screenshot(&out_path, 165);
        return;
    }

    if args.iter().any(|a| a == "--headless" || a == "--bench") {
        run_headless_benchmark();
        return;
    }

    println!("Launching Dzog Crystalline Corridor (Shmup) GUI...");
    gui::launch_gui();
}

fn capture_headless_screenshot(output_path: &str, steps: usize) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut game = ShmupFieldGame::new(
        ShmupFieldGame::DEFAULT_WIDTH,
        ShmupFieldGame::DEFAULT_HEIGHT,
    );
    game.autopilot_enabled = true;
    for _ in 0..steps {
        game.step();
    }
    game.render_to_pixel_buffer();

    let scale = 2usize; // 960x540 * 2 = 1920x1080 (Full HD 1080p)
    let rw = game.render_width();
    let rh = game.render_height();
    let out_w = rw * scale;
    let out_h = rh * scale;
    let mut upscaled = vec![0u8; out_w * out_h * 4];
    for y in 0..rh {
        for x in 0..rw {
            let src = (y * rw + x) * 4;
            let px = [
                game.pixel_buffer[src],
                game.pixel_buffer[src + 1],
                game.pixel_buffer[src + 2],
                255,
            ];
            for dy in 0..scale {
                let row = (y * scale + dy) * out_w + x * scale;
                for dx in 0..scale {
                    let dst = (row + dx) * 4;
                    upscaled[dst..dst + 4].copy_from_slice(&px);
                }
            }
        }
    }

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{out_w}x{out_h}"),
            "-i",
            "-",
            "-frames:v",
            "1",
            output_path,
        ])
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ffmpeg for screenshot");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&upscaled);
    }
    let _ = child.wait();
    println!("Saved lossless screenshot to {output_path} ({out_w}x{out_h})");
}

/// Controlled test setup verifying building wreckage (`GroundedWreckage`) & craters.
fn capture_wreckage_test_screenshot(output_path: &str) {
    let mut game = ShmupFieldGame::new(
        ShmupFieldGame::DEFAULT_WIDTH,
        ShmupFieldGame::DEFAULT_HEIGHT,
    );
    game.autopilot_enabled = true;
    for _ in 0..85 {
        game.step();
    }
    save_game_png(&mut game, output_path);
    println!(
        "Saved wreckage verification screenshot to {output_path} (ground_scars={})",
        game.ground_scars_count
    );
}

fn save_game_png(game: &mut ShmupFieldGame, output_path: &str) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    game.render_to_pixel_buffer();
    let scale = 2usize;
    let rw = game.render_width();
    let rh = game.render_height();
    let out_w = rw * scale;
    let out_h = rh * scale;
    let mut upscaled = vec![0u8; out_w * out_h * 4];
    for y in 0..rh {
        for x in 0..rw {
            let src = (y * rw + x) * 4;
            let px = [
                game.pixel_buffer[src],
                game.pixel_buffer[src + 1],
                game.pixel_buffer[src + 2],
                255,
            ];
            for dy in 0..scale {
                let row = (y * scale + dy) * out_w + x * scale;
                for dx in 0..scale {
                    let dst = (row + dx) * 4;
                    upscaled[dst..dst + 4].copy_from_slice(&px);
                }
            }
        }
    }

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{out_w}x{out_h}"),
            "-i",
            "-",
            "-frames:v",
            "1",
            output_path,
        ])
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to spawn ffmpeg");
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&upscaled);
    }
    let _ = child.wait();
}

/// Run isolated toy scenarios to self-evaluate each visual/physics feature without scene clutter:
/// 1) `toy_1_player_clean.png`: Player ship moving & firing bullets + missiles (verifies zero waves around player).
/// 2) `toy_2_shadows_and_roof.png`: Isolated GeodesicFuelDome, BiodomeReactor, and FlakTurret with horizon shadows.
/// 3) `toy_3_wreckage_and_splash.png`: Isolated destroyed building crater & grounded building rubble.
fn capture_isolated_toy_scenarios(out_dir: &str) {
    use dzog_core::{CohesiveBody, MineralClass};
    use entities::{GroundTarget, GroundTargetKind, GroundedWreckage, ScorchedCrater};

    // Toy Scenario 1: Player moving & firing over clean terrain/canal (zero enemy clutter)
    {
        let mut game = ShmupFieldGame::new(
            ShmupFieldGame::DEFAULT_WIDTH,
            ShmupFieldGame::DEFAULT_HEIGHT,
        );
        game.aliens.clear();
        game.ground_targets.clear();
        game.asteroids.clear();
        game.power_cubes.clear();
        game.solver.state.clear();

        for step_i in 0..18 {
            game.thrust_planar(if step_i < 9 { -0.8 } else { 0.8 }, -0.4);
            if step_i % 6 == 0 {
                game.fire_soliton_cannons(false);
            }
            if step_i == 8 {
                game.fire_air_to_ground_missiles();
            }
            game.step();
            game.aliens.clear();
            game.ground_targets.clear();
            game.asteroids.clear();
        }
        save_game_png(&mut game, &format!("{out_dir}/toy_1_player_clean.png"));
    }

    // Toy Scenario 2: Distinct Ship & Building Cracking + Enemy-to-Enemy Non-Overlap Bump
    {
        let mut game = ShmupFieldGame::new(
            ShmupFieldGame::DEFAULT_WIDTH,
            ShmupFieldGame::DEFAULT_HEIGHT,
        );
        game.aliens.clear();
        game.ground_targets.clear();
        game.asteroids.clear();
        game.power_cubes.clear();
        game.solver.state.clear();
        game.in_play = false;

        let mut d_hull =
            CohesiveBody::new_geodesic_dome(501, 68.0, 62.0, 6.8, MineralClass::BismuthCrystal);
        d_hull.inflict_progressive_fracture(67.0, 62.0, 0.5, -0.8, 1.45, None);
        for (&occ, c) in d_hull.occupied.iter().zip(d_hull.cohesion.iter_mut()) {
            if occ && *c < 0.22 {
                *c = 0.22;
            }
        }
        game.ground_targets.push(GroundTarget {
            kind: GroundTargetKind::GeodesicFuelDome,
            hull: d_hull,
            fire_cooldown: 999,
            fracture_hits: 2,
            pipeline_group: 1,
        });

        let mut b_hull =
            CohesiveBody::new_geodesic_dome(502, 172.0, 60.0, 7.6, MineralClass::PiezoQuartz);
        b_hull.inflict_progressive_fracture(171.0, 60.0, -0.6, -0.8, 1.55, None);
        for (&occ, c) in b_hull.occupied.iter().zip(b_hull.cohesion.iter_mut()) {
            if occ && *c < 0.22 {
                *c = 0.22;
            }
        }
        game.ground_targets.push(GroundTarget {
            kind: GroundTargetKind::BiodomeReactor,
            hull: b_hull,
            fire_cooldown: 999,
            fracture_hits: 2,
            pipeline_group: 1,
        });

        // Two TwinGunships that bumped off each other with distinct progressive hull cracking
        let mut g1 = CohesiveBody::new_twin_gunship(503, 108.0, 54.0, 6.8);
        g1.theta = std::f32::consts::FRAC_PI_2 - 0.22;
        g1.inflict_progressive_fracture(106.5, 54.0, 0.2, -1.0, 1.45, None);
        game.aliens.push(entities::AlienVessel {
            role: entities::AlienRole::TwinGunship,
            hull: g1,
            fire_cooldown: 999,
            carrier_phase: 0.0,
            cubes_healed: 0,
            fracture_hits: 2,
            cleave_count: 0,
        });

        let mut g2 = CohesiveBody::new_twin_gunship(504, 130.0, 52.0, 6.8);
        g2.theta = std::f32::consts::FRAC_PI_2 + 0.24;
        g2.inflict_progressive_fracture(131.5, 52.0, -0.2, -1.0, 1.55, None);
        game.aliens.push(entities::AlienVessel {
            role: entities::AlienRole::TwinGunship,
            hull: g2,
            fire_cooldown: 999,
            carrier_phase: 0.0,
            cubes_healed: 0,
            fracture_hits: 2,
            cleave_count: 0,
        });

        game.sync_all_bodies_to_lattice();
        save_game_png(&mut game, &format!("{out_dir}/toy_2_shadows_and_roof.png"));
    }

    // Toy Scenario 3: Field-Driven Fiery Explosions & Rolling Dark Smoke (3 stages of evolution)
    {
        let mut game = ShmupFieldGame::new(
            ShmupFieldGame::DEFAULT_WIDTH,
            ShmupFieldGame::DEFAULT_HEIGHT,
        );
        game.aliens.clear();
        game.ground_targets.clear();
        game.asteroids.clear();
        game.power_cubes.clear();
        game.solver.state.clear();
        game.in_play = false;

        // Stage A (Right, x=172, y=64): Older blast that has diffused into billowing dark smoke over a crater & rubble
        game.craters.push(ScorchedCrater {
            x: 172.0,
            y: 64.0,
            radius: 7.5,
            ember_heat: 0.9,
        });
        let rubble =
            CohesiveBody::new_asteroid(703, 175.0, 66.0, 4.2, 6, 33, MineralClass::ObsidianAnvil);
        game.grounded_wreckage.push(GroundedWreckage {
            body: rubble,
            z: 0.48,
            skid_start_x: 170.0,
            skid_start_y: 62.0,
            ember_heat: 1.0,
        });
        game.inject_fiery_field_explosion(172.0, 64.0, 5.5, 2.2);
        for _ in 0..8 {
            game.gpu_accel.step(&mut game.solver, 4);
        }

        // Stage B (Center, x=120, y=58): Mid-stage expanding orange/crimson fireball with outer dark smoke rim
        game.inject_fiery_field_explosion(120.0, 58.0, 6.0, 3.4);
        for _ in 0..3 {
            game.gpu_accel.step(&mut game.solver, 4);
        }

        // Stage C (Left, x=70, y=60): Fresh white-hot detonation core + fiery orange lobes
        game.craters.push(ScorchedCrater {
            x: 70.0,
            y: 60.0,
            radius: 6.8,
            ember_heat: 1.0,
        });
        game.inject_fiery_field_explosion(70.0, 60.0, 6.2, 4.0);

        game.sync_all_bodies_to_lattice();
        save_game_png(&mut game, &format!("{out_dir}/toy_3_wreckage_and_splash.png"));
    }

    println!("Saved 3 isolated toy scenario screenshots to {out_dir}/toy_{{1,2,3}}_*.png");
}

fn run_headless_benchmark() {
    println!("=== Dzog Shmup Performance Profile (300 frames) ===");
    let mut game = ShmupFieldGame::new(
        ShmupFieldGame::DEFAULT_WIDTH,
        ShmupFieldGame::DEFAULT_HEIGHT,
    );
    game.autopilot_enabled = true;

    let mut step_time = std::time::Duration::ZERO;
    let mut render_time = std::time::Duration::ZERO;

    for _ in 0..300 {
        let ts = Instant::now();
        game.step();
        step_time += ts.elapsed();

        let tr = Instant::now();
        game.render_to_pixel_buffer();
        render_time += tr.elapsed();
    }
    let total = step_time + render_time;
    println!(
        "  300 frames total: {:.2?} ({:.1} FPS avg)\n  - step():   {:.2?} ({:.2?}/frame)\n  - render(): {:.2?} ({:.2?}/frame)",
        total,
        300.0 / total.as_secs_f64(),
        step_time,
        step_time / 300,
        render_time,
        render_time / 300,
    );
}
