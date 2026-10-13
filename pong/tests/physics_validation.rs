//! Automated field-first physics validation tests for Breakout on the Dzog lattice.

#[path = "../src/field_game.rs"]
mod field_game;

use field_game::{BreakoutFieldGame, StepEvent};

#[test]
fn test_breakout_paddle_rebound_and_serve() {
    let mut game = BreakoutFieldGame::new(80, 48);

    assert!(game.ball_attached, "Ball should start attached to paddle");
    game.serve();
    assert!(!game.ball_attached, "Ball should be launched after serve");
    assert!(game.ball_vy < 0.0, "Ball must travel upwards toward bricks");

    // Position paddle to intercept a downward falling ball
    game.ball_x = 40.0;
    game.ball_y = game.paddle_y as f32 - 4.0;
    game.ball_vx = 0.0;
    game.ball_vy = 0.6; // Moving down towards paddle

    game.paddle_x = 30.0;
    game.sync_paddle_boundaries();

    let mut rebounded = false;
    for _ in 0..60 {
        let event = game.step();
        if event == StepEvent::PaddleHit || game.ball_vy < 0.0 {
            rebounded = true;
            break;
        }
    }

    assert!(rebounded, "Ball should rebound upwards off paddle");
    assert!(game.ball_vy < 0.0, "Ball vertical velocity must be inverted");

    // Verify acoustic lattice was excited at paddle strike point
    let p_max = game.solver.state.pressure.iter().copied().fold(0.0f32, f32::max);
    assert!(p_max > 0.05, "Paddle strike must excite acoustic shockwaves");
}

#[test]
fn test_two_hit_brick_destruction() {
    let mut game = BreakoutFieldGame::new(80, 48);
    let initial_bricks = game.bricks_remaining;
    assert!(initial_bricks > 0);

    // Identify bottom-row brick (row 3, col 0) so the ball has an open path from below
    let target_idx = 24;
    let target = game.bricks[target_idx];
    let brick_cx = target.x as f32 + target.w as f32 / 2.0;
    let brick_bottom = (target.y + target.h) as f32;

    // --- HIT 1: STRESS CRACK ---
    game.ball_attached = false;
    game.ball_x = brick_cx;
    game.ball_y = brick_bottom + 2.0;
    game.ball_vx = 0.0;
    game.ball_vy = -0.6; // Travel straight up into brick

    let mut cracked = false;
    for _ in 0..40 {
        let event = game.step();
        if let StepEvent::BrickCracked { brick_idx, .. } = event {
            assert_eq!(brick_idx, target_idx);
            cracked = true;
            break;
        }
    }

    assert!(cracked, "First hit must trigger BrickCracked event");
    assert_eq!(game.bricks[target_idx].hits_left, 1, "Brick should have 1 hit remaining");
    assert!(game.bricks[target_idx].is_alive(), "Brick must remain alive after 1 hit");
    assert_eq!(game.bricks_remaining, initial_bricks, "Brick count must not decrease yet");
    assert!(game.ball_vy > 0.0, "Ball must rebound downward after hit 1");

    // --- HIT 2: PHYSICAL SHATTER ---
    // Launch again into the same cracked brick
    game.ball_x = brick_cx;
    game.ball_y = brick_bottom + 2.0;
    game.ball_vx = 0.0;
    game.ball_vy = -0.6; // Travel straight up into brick

    let mut broken = false;
    for _ in 0..40 {
        let event = game.step();
        if let StepEvent::BrickBroken { brick_idx, .. } = event {
            assert_eq!(brick_idx, target_idx);
            broken = true;
            break;
        }
    }

    assert!(broken, "Second hit must physically break the brick");
    assert_eq!(game.bricks[target_idx].hits_left, 0, "Brick hits_left should be 0");
    assert!(!game.bricks[target_idx].is_alive(), "Brick must be dead after 2 hits");
    assert_eq!(game.bricks_remaining, initial_bricks - 1, "Brick count must decrement by 1");

    // Verify boundary was cleared from Yee lattice
    let b = game.bricks[target_idx];
    let idx_center = game.solver.state.config.index(b.x + b.w / 2, b.y + b.h / 2);
    assert!(!game.solver.boundary.wall_east[idx_center], "Broken brick boundary must be cleared on East face");
    assert!(!game.solver.boundary.wall_north[idx_center], "Broken brick boundary must be cleared on North face");

    // Verify cavitation blast acoustic shockwave
    let p_max = game.solver.state.pressure.iter().copied().fold(0.0f32, f32::max);
    assert!(p_max > 0.2, "Cavitation rupture must inject massive pressure burst");
}

#[test]
fn test_blast_toggle_disables_shockwave_and_force() {
    let mut game = BreakoutFieldGame::new(80, 48);
    game.wave_force_enabled = false; // Toggle OFF (Counterfactual decoupled mode)
    game.solver.state.clear();

    // Brick 24 is at (9, 31), well inside the 80x48 arena
    let target_idx = 24;
    let target = game.bricks[target_idx];
    let brick_cx = target.x as f32 + target.w as f32 / 2.0;
    let brick_bottom = (target.y + target.h) as f32;

    // Force brick to 1 hit left so next hit shatters it
    game.bricks[target_idx].hits_left = 1;
    game.ball_attached = false;
    game.ball_x = brick_cx;
    game.ball_y = brick_bottom + 2.0;
    game.ball_vx = 0.0;
    game.ball_vy = -0.6;

    let mut broken = false;
    for _ in 0..30 {
        if let StepEvent::BrickBroken { .. } = game.step() {
            broken = true;
            break;
        }
    }

    assert!(broken, "Brick should still break kinematically when wave_force_enabled is OFF");
    let p_max = game.solver.state.pressure.iter().copied().fold(0.0f32, |a, b| a.max(b.abs()));
    assert_eq!(
        p_max, 0.0,
        "When wave_force_enabled is OFF, zero acoustic blast or wake should be injected (got {p_max})"
    );
}

#[test]
fn test_point_of_impact_directional_fracturing() {
    let mut b_left = field_game::Brick::new(10, 10, 12, 4, 0);
    let mut b_right = field_game::Brick::new(10, 10, 12, 4, 0);

    // Strike b_left on its left edge (x = 10.0, y = 12.0, outward normal = (-1, 0))
    b_left.apply_contact_fracture(10.0, 12.0, -1.0, 0.0);

    // Strike b_right on its right edge (x = 21.0, y = 12.0, outward normal = (1, 0))
    b_right.apply_contact_fracture(21.0, 12.0, 1.0, 0.0);

    let left_edge_dmg_on_b_left: f32 = (0..4).map(|ry| b_left.damage[ry][0]).sum();
    let right_edge_dmg_on_b_left: f32 = (0..4).map(|ry| b_left.damage[ry][11]).sum();

    let left_edge_dmg_on_b_right: f32 = (0..4).map(|ry| b_right.damage[ry][0]).sum();
    let right_edge_dmg_on_b_right: f32 = (0..4).map(|ry| b_right.damage[ry][11]).sum();

    assert!(
        left_edge_dmg_on_b_left > right_edge_dmg_on_b_left,
        "Left-edge impact must concentrate fracture damage on left cells"
    );
    assert!(
        right_edge_dmg_on_b_right > left_edge_dmg_on_b_right,
        "Right-edge impact must concentrate fracture damage on right cells"
    );
}

#[test]
fn test_acoustic_wave_resonance_cleaving() {
    let mut game = BreakoutFieldGame::new(128, 80);
    game.wave_force_enabled = true;

    // Select a bottom-row Sonic Resonator brick (row 3) and pre-crack it (hits_left = 1)
    let target_idx = 24;
    let target = game.bricks[target_idx];
    let cx = target.x as f32 + target.w as f32 * 0.5;
    let cy = target.y as f32 + target.h as f32 * 0.5;

    game.bricks[target_idx].hits_left = 1;
    game.bricks[target_idx].apply_contact_fracture(cx, cy + 2.0, 0.0, 1.0);
    let b_copy = game.bricks[target_idx];
    game.sync_single_brick_to_lattice(&b_copy);

    let initial_remaining = game.bricks_remaining;

    // Keep ball attached far away at the paddle so it cannot touch the brick
    assert!(game.ball_attached);

    // Inject a constructive acoustic shockwave right next to the cracked brick
    game.inject_mouse_impulse(cx, cy + 2.0, 3.8);
    game.step();

    assert!(
        !game.bricks[target_idx].is_alive(),
        "Cracked brick should be cleaved purely by the acoustic shockwave without ball contact"
    );
    assert_eq!(game.bricks_remaining, initial_remaining - 1);
    assert!(game.acoustic_cleaves >= 1);
}

#[test]
fn test_counterfactual_branch_checkpoint_and_views() {
    let mut game = BreakoutFieldGame::new(128, 80);
    game.serve();
    for _ in 0..5 {
        game.step();
    }

    // Save counterfactual branch checkpoint
    game.save_branch_checkpoint();
    let saved_ball_x = game.ball_x;
    let saved_ball_y = game.ball_y;
    let saved_energy = game.field_energy.total;

    // Advance 20 more steps and cycle view modes
    for _ in 0..20 {
        game.step();
        game.cycle_view_mode();
        game.render_to_pixel_buffer();
    }
    assert_ne!(game.ball_y, saved_ball_y);

    // Restore branch checkpoint
    assert!(game.restore_branch_checkpoint());
    assert_eq!(game.ball_x, saved_ball_x);
    assert_eq!(game.ball_y, saved_ball_y);
    assert!((game.field_energy.total - saved_energy).abs() < 1e-5);
}

#[test]
fn test_acoustic_radiation_force_coupling() {
    let mut game = BreakoutFieldGame::new(64, 40);
    game.ball_attached = false;
    game.ball_x = 32.0;
    game.ball_y = 20.0;
    game.ball_vx = 0.0;
    game.ball_vy = 0.0;
    game.wave_force_enabled = true;

    // Inject strong high pressure compression to the right of the ball (at x = 33)
    let idx_east = game.solver.state.config.index(33, 20);
    game.solver.state.pressure[idx_east] = 5.0;

    // Step simulation: the negative pressure gradient -grad(p) should push ball leftwards (vx < 0)
    game.step();

    assert!(game.ball_vx < 0.0, "Ball should be accelerated away from high acoustic pressure by radiation force");
}

#[test]
fn test_pixel_rasterization_and_energy() {
    let mut game = BreakoutFieldGame::new(64, 32);
    game.serve();
    game.render_to_pixel_buffer();

    assert_eq!(game.pixel_buffer.len(), 64 * 32 * 4);

    // Non-zero pixels exist (ball, paddle, walls, bricks, wave field)
    let non_zero_count = game.pixel_buffer.iter().filter(|&&b| b > 0).count();
    assert!(non_zero_count > 300, "Pixel buffer should contain rasterized game field");

    // Hamiltonian field energy
    assert!(game.field_energy.total >= 0.0, "Total field energy must be non-negative");
}

#[test]
fn test_no_one_hit_self_cleave_on_center_or_corner_strike() {
    let mut game = BreakoutFieldGame::new(128, 80);
    game.wave_force_enabled = true;

    // Strike brick 24 on its extreme right corner (the worst case for the old self-cleave bug)
    let target_idx = 24;
    let target = game.bricks[target_idx];
    let corner_x = (target.x + target.w) as f32;
    let brick_bottom = (target.y + target.h) as f32;

    game.ball_attached = false;
    game.ball_x = corner_x;
    game.ball_y = brick_bottom + 1.8;
    game.ball_vx = -0.15;
    game.ball_vy = -0.65;

    let mut cracked = false;
    for _ in 0..35 {
        let ev = game.step();
        if let StepEvent::BrickCracked { brick_idx, .. } = ev {
            if brick_idx == target_idx {
                cracked = true;
            }
        }
        // As the ball rebounds away, the brick's own Hit-1 impact vibration must NEVER self-break it!
        assert_ne!(
            ev,
            StepEvent::BrickBroken {
                brick_idx: target_idx,
                row: target.row,
                remaining: 31
            },
            "Brick must never self-break from its own Hit-1 impact wave!"
        );
    }

    assert!(cracked, "Corner strike must crack the brick on Hit 1");
    assert_eq!(
        game.bricks[target_idx].hits_left, 1,
        "Brick must strictly remain at hits_left == 1 after rebounding away from Hit 1"
    );
}

#[test]
fn test_inter_brick_phonon_stress_transfer() {
    let mut game = BreakoutFieldGame::new(128, 80);
    let target_idx = 24;
    let right_neighbor_idx = 25;

    let initial_neighbor_cohesion = game.bricks[right_neighbor_idx].mean_cohesion();
    assert_eq!(initial_neighbor_cohesion, 1.0);

    // Transfer solid-state phonon stress from brick 24 to its neighbors
    game.propagate_phonon_stress_to_neighbors(target_idx, 0.35);

    let post_neighbor_cohesion = game.bricks[right_neighbor_idx].mean_cohesion();
    assert!(
        post_neighbor_cohesion < initial_neighbor_cohesion,
        "Solid-state phonon coupling must pre-stress adjacent brick's facing edge"
    );
}

#[test]
fn test_paddle_doppler_overcharge_and_phase_pulse() {
    let mut game = BreakoutFieldGame::new(128, 80);

    // 1. Test coherent Phase-Pulse [W] discharge from paddle
    assert!(game.phase_charge >= 30.0);
    let fired = game.fire_phase_pulse();
    assert!(fired, "Phase pulse should fire when charge >= 30");
    let max_p = game.solver.state.pressure.iter().copied().fold(0.0f32, f32::max);
    assert!(max_p > 1.5, "Phase pulse must inject directional wave packet above paddle");

    // 2. Test Overcharged Ball single-hit shatter
    let target_idx = 26;
    let target = game.bricks[target_idx];
    assert_eq!(target.hits_left, 2);

    game.ball_attached = false;
    game.ball_overcharged = true;
    game.ball_x = target.x as f32 + target.w as f32 * 0.5;
    game.ball_y = (target.y + target.h) as f32 + 1.8;
    game.ball_vx = 0.0;
    game.ball_vy = -0.75;

    let mut shattered_in_one_hit = false;
    for _ in 0..15 {
        if let StepEvent::BrickBroken { brick_idx, .. } = game.step() {
            if brick_idx == target_idx {
                shattered_in_one_hit = true;
                break;
            }
        }
    }

    assert!(
        shattered_in_one_hit,
        "Overcharged ball from Doppler paddle slice must shatter an intact crystal brick in 1 hit"
    );
    assert_eq!(game.bricks[target_idx].hits_left, 0);
}

#[test]
fn test_gpu_accelerated_breakout_stepping() {
    let mut game = BreakoutFieldGame::new(64, 40);
    if !game.enable_gpu_acceleration() {
        return;
    }
    assert!(game.is_gpu_accelerated());

    game.serve();
    for _ in 0..10 {
        game.step();
    }

    assert!(
        game.field_energy.total > 0.0,
        "GPU-accelerated Breakout stepping must evolve field state and energy"
    );
}
