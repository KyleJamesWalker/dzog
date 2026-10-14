#[path = "../src/autopilot.rs"]
pub mod autopilot;
#[path = "../src/canyon.rs"]
pub mod canyon;
#[path = "../src/entities.rs"]
pub mod entities;
#[path = "../src/field_game.rs"]
pub mod field_game;

use entities::ShmupEvent;
use field_game::ShmupFieldGame;

#[test]
fn test_trilobe_ship_takes_three_hits_and_power_cubes_heal_player_or_aliens() {
    let mut game = ShmupFieldGame::new(240, 150);
    assert_eq!(game.player_intact_lobes(), 3);
    assert!(game.in_play);

    // 1. Hit 1 severs Port Wing -> 2 lobes remain, ship still in play
    let ev1 = game.damage_player_lobe(game.player_hull.x - 3.0);
    assert_eq!(ev1, ShmupEvent::PlayerLobeCleaved { remaining_lobes: 2 });
    assert_eq!(game.player_intact_lobes(), 2);
    assert!(game.in_play);

    // 2. Hit 2 severs Starboard Wing -> 1 Central Core lobe remains, ship still in play
    let ev2 = game.damage_player_lobe(game.player_hull.x + 3.0);
    assert_eq!(ev2, ShmupEvent::PlayerLobeCleaved { remaining_lobes: 1 });
    assert_eq!(game.player_intact_lobes(), 1);
    assert!(game.in_play);

    // 3. Spawn two Annealing Power Cubes directly on the player ship -> regrows severed wing lobes!
    game.power_cubes.clear();
    let px = game.player_hull.x;
    let py = game.player_hull.y;
    game.spawn_power_cube(px, py, 0.0, 0.0);
    game.spawn_power_cube(px, py, 0.0, 0.0);
    game.step();
    assert!(
        game.cubes_collected_player >= 2,
        "Player must absorb nearby Annealing Power Cubes"
    );
    assert!(
        game.player_intact_lobes() >= 2,
        "Power Cubes must anneal crystal cohesion and regrow severed wing lobes (got {})",
        game.player_intact_lobes()
    );

    // 4. Verify damaged alien ship can also pick up a Power Cube to self-heal once inside the corridor
    game.aliens[0].hull.y = 32.0;
    let ax = game.aliens[0].hull.x;
    let ay = game.aliens[0].hull.y;
    game.aliens[0]
        .hull
        .carve_cleavage_fissure(ax, ay, 0.0, 1.0, 1.0, None);
    let coh_damaged = game.aliens[0].hull.mean_cohesion();
    assert!(coh_damaged < 0.96, "Alien hull should have eroded cohesion after strike");

    game.spawn_power_cube(ax, ay, 0.0, 0.0);
    game.step();
    assert!(
        game.cubes_collected_aliens >= 1,
        "Damaged alien vessel must capture overlapping Power Cube"
    );
    assert!(
        game.aliens[0].hull.mean_cohesion() > coh_damaged,
        "Power Cube must anneal and increase alien hull cohesion"
    );

    // 5. Verify 3 consecutive structural lobe hits cavitate the player core and end the run
    game.reset();
    assert_eq!(game.player_intact_lobes(), 3);
    game.damage_player_lobe(game.player_hull.x - 3.0);
    assert_eq!(game.player_intact_lobes(), 2);
    game.damage_player_lobe(game.player_hull.x + 3.0);
    assert_eq!(game.player_intact_lobes(), 1);
    game.damage_player_lobe(game.player_hull.x);
    assert_eq!(game.player_intact_lobes(), 0);
    assert!(!game.in_play, "Third structural lobe hit must cavitate core and end game");
}

#[test]
fn test_chladni_swarm_and_wasd_soliton_cannons_field_coupling() {
    let mut game = ShmupFieldGame::new(240, 150);

    // Verify player starts near bottom of screen and enemies spawn offscreen at top (y < 0)
    assert!(
        game.player_hull.y > 100.0,
        "Player ship must start near bottom of screen (got y={})",
        game.player_hull.y
    );
    assert!(
        game.aliens.iter().all(|a| a.hull.y < 0.0 && a.hull.vy > 0.0),
        "Alien swarm must spawn offscreen above top horizon (y < 0) and flow downward (+vy > 0)"
    );

    // 1. 4-Way WASD planar thrust upward (-Y) injects reaction plume and moves ship toward top
    let y0 = game.player_hull.y;
    game.thrust_planar(0.0, -1.0);
    game.fire_soliton_cannons(false);
    assert_eq!(
        game.bolts.len(),
        3,
        "Intact 3-lobe ship must fire 3 soliton bolts (Core + Port Wing + Starboard Wing)"
    );
    assert!(
        game.bolts.iter().all(|b| b.vy < -1.5),
        "Player soliton bolts must travel fast upward (-Y)"
    );

    for _ in 0..10 {
        game.step();
    }
    assert!(
        game.player_hull.y < y0,
        "Upward planar thrust (-Y) must move player ship toward top of screen"
    );
    assert!(
        game.field_energy.total > 0.1,
        "Chladni swarm carrier, thrusters, and soliton cannons must excite Hamiltonian field energy"
    );

    // 2. Counterfactual branching roundtrip
    game.save_branch_checkpoint();
    let saved_score = game.score;
    game.score += 999;
    assert!(game.restore_branch_checkpoint());
    assert_eq!(game.score, saved_score);
}
