//! Automated field-first physics validation tests for Pong on the Dzog lattice.

#[path = "../src/field_game.rs"]
mod field_game;

use field_game::SolitonFieldGame;

#[test]
fn test_field_pong_paddle_rebound_and_rally() {
    let mut game = SolitonFieldGame::new(80, 32);

    // Serve horizontally towards player 2 (right paddle at x = 72)
    game.serve(2);
    game.ball_vy = 0.0; // Launch horizontally dead center into paddle
    assert!(game.in_play);

    // Ensure ball starts in court center
    assert!(game.ball_x >= 38.0 && game.ball_x <= 42.0);
    assert!(game.ball_vx > 0.0);

    // Position player 2 paddle to guarantee interception
    game.p2_paddle_y = game.height / 2 - game.paddle_height / 2;
    game.sync_paddle_boundaries();

    // Step until ball hits paddle 2 and reflects leftwards
    let mut rebounded = false;
    for _ in 0..80 {
        game.step();
        if game.ball_vx < 0.0 {
            rebounded = true;
            break;
        }
    }

    assert!(rebounded, "Ball should have reflected off paddle 2");
    assert!(game.in_play, "Game should remain in play during rally");
    assert_eq!(game.rally_count, 1, "Rally count should increment on paddle hit");

    // Verify acoustic lattice was excited at impact point
    let p_max = game.solver.state.pressure.iter().copied().fold(0.0f32, f32::max);
    assert!(p_max > 0.1, "Paddle bounce must excite acoustic lattice waves");
}

#[test]
fn test_field_pong_goal_detection() {
    let mut game = SolitonFieldGame::new(80, 32);

    // Serve towards player 1 (left)
    game.serve(1);
    assert!(game.in_play);

    // Move player 1's paddle out of the way to allow goal
    game.p1_paddle_y = 1;
    game.sync_paddle_boundaries();

    // Step until ball reaches left goal line (x < 2)
    let mut scored = false;
    for _ in 0..120 {
        if let Some(scorer) = game.step() {
            assert_eq!(scorer, 2, "Player 2 should score when ball enters left goal");
            scored = true;
            break;
        }
    }

    assert!(scored, "Goal should trigger when ball penetrates past paddle line");
    assert_eq!(game.p2_score, 1, "Player 2 score should increment");
    assert!(!game.in_play, "Game should exit in_play after goal");
}

#[test]
fn test_field_pixel_rasterization() {
    let mut game = SolitonFieldGame::new(64, 32);
    game.serve(2);
    game.render_to_pixel_buffer();

    assert_eq!(game.pixel_buffer.len(), 64 * 32 * 4);

    // Verify non-zero pixels exist (ball, paddles, boundary, waves)
    let non_zero_count = game.pixel_buffer.iter().filter(|&&b| b > 0).count();
    assert!(non_zero_count > 100, "Pixel buffer should contain rasterized game field");
}
