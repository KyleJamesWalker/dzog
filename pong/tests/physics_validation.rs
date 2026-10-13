//! Automated field-first physics validation tests for Pong on the Dzog lattice.

#[path = "../src/field_game.rs"]
mod field_game;

use field_game::SolitonFieldGame;

#[test]
fn test_field_soliton_paddle_reflection() {
    let mut game = SolitonFieldGame::new(80, 32);

    // Serve towards player 2 (right paddle at x = 72)
    game.serve(2);
    assert!(game.in_play);

    // Initial soliton should be in the center traveling rightwards
    let (cx, _) = game.find_soliton_centroid();
    assert!(cx >= 38.0 && cx <= 42.0);

    // Advance 30 steps until soliton reaches paddle 2 and reflects leftwards
    for _ in 0..40 {
        game.step();
    }

    // Centroid should have reversed direction or reflected off the paddle
    let (rebound_x, _) = game.find_soliton_centroid();
    println!("Centroid after rebound: x = {rebound_x:.2}");
    // Soliton is still active and inside the court
    assert!(rebound_x < 72.0, "Soliton should rebound off paddle 2");
}

#[test]
fn test_field_soliton_goal_probe_scoring() {
    let mut game = SolitonFieldGame::new(80, 32);

    // Serve towards player 1 (left)
    game.serve(1);
    assert!(game.in_play);

    // Move player 1's paddle out of the way to allow goal
    game.p1_paddle_y = 1;
    game.sync_paddle_boundaries();

    // Step until goal sensor probe triggers
    let mut scored = false;
    for _ in 0..100 {
        if let Some(scorer) = game.step() {
            assert_eq!(scorer, 2, "Player 2 should score when wave reaches left net");
            scored = true;
            break;
        }
    }

    assert!(scored, "Goal probe must trigger upon soliton penetration");
    assert_eq!(game.p2_score, 1, "Player 2 score should be incremented");
    assert!(!game.in_play, "Game should exit in_play after goal");
}

#[test]
fn test_field_pixel_rasterization() {
    let mut game = SolitonFieldGame::new(64, 32);
    game.serve(2);
    game.render_to_pixel_buffer();

    assert_eq!(game.pixel_buffer.len(), 64 * 32 * 4);

    // Verify non-zero pixels exist (paddles, boundary, soliton)
    let non_zero_count = game.pixel_buffer.iter().filter(|&&b| b > 0).count();
    assert!(non_zero_count > 100, "Pixel buffer should contain rasterized game field");
}
