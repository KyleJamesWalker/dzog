//! Automated physical accuracy validation tests for Pong against Dzog engine progress.

#[path = "../src/field_pong.rs"]
mod field_pong;

#[path = "../src/rigid_pong.rs"]
mod rigid_pong;

use field_pong::FieldWavePong;
use rigid_pong::RigidPong;

#[test]
fn test_pong_elastic_collision_energy_conservation() {
    let mut pong = RigidPong::new(60, 24);
    pong.restitution = 1.0; // Perfectly elastic collision

    // Fire ball horizontally directly into paddle 2 (x = 55)
    pong.ball_x = 30.0;
    pong.ball_y = 12.0;
    pong.ball_vx = 0.8;
    pong.ball_vy = 0.0;
    pong.in_play = true;

    let initial_ke = pong.kinetic_energy();
    assert!(initial_ke > 0.0);

    // Step until ball hits paddle 2 and bounces back
    let mut bounced = false;
    for _ in 0..60 {
        pong.step(1.0);
        if pong.ball_vx < 0.0 {
            bounced = true;
            break;
        }
    }

    assert!(bounced, "Ball should have bounced off paddle 2");
    let post_bounce_ke = pong.kinetic_energy();

    // In a perfectly elastic collision with stationary paddle, kinetic energy is strictly conserved
    let ke_drift = (post_bounce_ke - initial_ke).abs() / initial_ke;
    assert!(
        ke_drift < 1e-4,
        "Kinetic energy drift ({ke_drift}) must be near zero in elastic collision"
    );
}

#[test]
fn test_pong_moving_paddle_tangential_momentum_transfer() {
    let mut pong = RigidPong::new(60, 24);
    pong.ball_x = 53.0;
    pong.ball_y = 12.0;
    pong.ball_vx = 0.6;
    pong.ball_vy = 0.0;
    pong.p2_vy = 2.0; // Paddle 2 moving rapidly downwards
    pong.in_play = true;

    // Advance 4 steps so collision occurs
    for _ in 0..4 {
        pong.step(1.0);
    }

    // After collision with downwards moving paddle, ball must have acquired positive vy
    assert!(
        pong.ball_vy > 0.2,
        "Ball must acquire tangential velocity from moving paddle (got vy={})",
        pong.ball_vy
    );
}

#[test]
fn test_pong_acoustic_field_excitation_on_impact() {
    let mut pong = RigidPong::new(60, 24);
    pong.ball_x = 53.0;
    pong.ball_y = 12.0;
    pong.ball_vx = 0.8;
    pong.ball_vy = 0.0;
    pong.in_play = true;

    // Check acoustic field energy before impact
    let pre_energy = dzog_core::FieldEnergy::compute(&pong.solver.state, &pong.solver.material);
    assert_eq!(pre_energy.total, 0.0);

    // Step through paddle impact
    for _ in 0..5 {
        pong.step(1.0);
    }

    assert!(pong.total_impacts >= 1, "Should have registered an impact");

    // Check acoustic field energy after impact
    let post_energy = dzog_core::FieldEnergy::compute(&pong.solver.state, &pong.solver.material);
    assert!(
        post_energy.total > 0.0,
        "Collision impact must excite acoustic field energy in the Dzog lattice"
    );
}

#[test]
fn test_pong_field_wave_goal_probe_detection() {
    let mut pong = FieldWavePong::new(60, 24);

    // Serve towards player 1 (left)
    pong.serve(1);
    assert!(pong.in_play);

    // Move player 1's paddle out of the way to allow goal
    pong.p1_paddle_y = 1;
    pong.update_paddles_in_field();

    // Advance steps until acoustic wave packet enters left goal net
    let mut scored = false;
    for _ in 0..120 {
        if let Some(scorer) = pong.step() {
            assert_eq!(scorer, 2, "Player 2 should score when wave reaches player 1 net");
            scored = true;
            break;
        }
    }

    assert!(scored, "Goal probe should detect acoustic wave packet penetration");
    assert_eq!(pong.p2_score, 1, "Player 2 score should be incremented");
    assert!(!pong.in_play, "Game should exit in_play on goal");
}
