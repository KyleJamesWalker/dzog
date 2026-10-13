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
