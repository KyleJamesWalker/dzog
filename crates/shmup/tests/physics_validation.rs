#[path = "../src/autopilot.rs"]
pub mod autopilot;
#[path = "../src/canyon.rs"]
pub mod canyon;
#[path = "../src/combat.rs"]
pub mod combat;
#[path = "../src/entities.rs"]
pub mod entities;
#[path = "../src/field_game.rs"]
pub mod field_game;
#[path = "../src/render/mod.rs"]
pub mod render;
#[path = "../src/spawning.rs"]
pub mod spawning;

use entities::{AlienRole, GroundTargetKind, ShmupEvent};
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

#[test]
fn test_progressive_enemy_fracturing_and_ground_waveguide_chain_detonation() {
    let mut game = ShmupFieldGame::new(240, 150);

    // 1. Locate the TwinGunship medium-large enemy and test progressive fracturing on repeated hits
    let gunship_idx = game
        .aliens
        .iter()
        .position(|a| a.role == AlienRole::TwinGunship)
        .expect("Wave must spawn a TwinGunship");
    game.aliens[gunship_idx].hull.x = 120.0;
    game.aliens[gunship_idx].hull.y = 45.0;

    let gx = game.aliens[gunship_idx].hull.x;
    let gy = game.aliens[gunship_idx].hull.y;
    let coh_initial = game.aliens[gunship_idx].hull.mean_cohesion();

    // Strike 1 on intact armor: carves initial fracture waveguides without full cleavage
    let (cleave1, prior_cracks1) = game.aliens[gunship_idx]
        .hull
        .inflict_progressive_fracture(gx - 2.5, gy, 0.0, -1.0, 1.35, None);
    let coh_after_hit1 = game.aliens[gunship_idx].hull.mean_cohesion();
    assert!(!cleave1, "First standard hit on intact gunship should fracture, not cleave");
    assert_eq!(prior_cracks1, 0);
    assert!(coh_after_hit1 < coh_initial);

    // Strike 2 on the same fracture zone: detects existing cracks and amplifies fracture network
    let (_cleave2, prior_cracks2) = game.aliens[gunship_idx]
        .hull
        .inflict_progressive_fracture(gx - 2.5, gy, 0.0, -1.0, 1.35, None);
    let coh_after_hit2 = game.aliens[gunship_idx].hull.mean_cohesion();
    assert!(
        prior_cracks2 >= 1,
        "Second strike on fractured zone must detect existing cracks"
    );
    assert!(
        coh_after_hit2 < coh_after_hit1,
        "Repeated strikes on fractures must progressively erode hull cohesion"
    );

    // Strike 3 on the deepened fracture: triggers full topological cleavage
    let (cleave3, prior_cracks3) = game.aliens[gunship_idx]
        .hull
        .inflict_progressive_fracture(gx - 2.5, gy, 0.0, -1.0, 1.45, None);
    assert!(cleave3 || prior_cracks3 >= 4);

    // 2. Test Alt-Fire Air-to-Ground Missiles (`fire_air_to_ground_missiles`):
    //    Missiles pass underneath sky enemies and detonate Ground GeodesicFuelDome -> ScorchedCrater + Pipeline Chain Surge
    let dome_idx = game
        .ground_targets
        .iter()
        .position(|g| g.kind == GroundTargetKind::GeodesicFuelDome)
        .expect("Wave must spawn a GeodesicFuelDome");
    let pipe_grp = game.ground_targets[dome_idx].pipeline_group;
    let dx = 52.0;
    let dy = 60.0;
    game.ground_targets[dome_idx].hull.x = dx;
    game.ground_targets[dome_idx].hull.y = dy;

    for gt in &mut game.ground_targets {
        if gt.hull.y < 0.0 {
            gt.hull.y = 55.0;
        }
    }

    // Position player below the dome and fire Alt-Fire Air-to-Ground Missiles
    game.player_hull.x = dx + 3.5;
    game.player_hull.y = dy + 4.0;
    game.bolts.clear();
    game.fire_air_to_ground_missiles();
    assert_eq!(
        game.bolts.len(),
        2,
        "Alt-fire must launch twin finned Air-to-Ground Missiles"
    );

    game.step();

    assert!(
        !game.craters.is_empty() && !game.grounded_wreckage.is_empty(),
        "Air-to-Ground Missile impact on a ground dome must stamp a ScorchedCrater and GroundedWreckage rubble"
    );
    assert!(
        game.credits >= 800,
        "Destroying ground infrastructure must award Raptor bounty credits ($)"
    );
    assert!(
        game.chain_detonations >= 1,
        "Detonating a pipeline-coupled dome (group {}) must surge connected ground targets",
        pipe_grp
    );

    // 3. Verify kinetic-force bullet impacts impart angular torque (`omega`) and knockback to enemies
    let g_idx = game
        .aliens
        .iter()
        .position(|a| a.role == AlienRole::TwinGunship)
        .expect("TwinGunship must still be present");
    let gx = game.aliens[g_idx].hull.x;
    let gy = game.aliens[g_idx].hull.y;
    game.bolts.push(entities::SolitonBolt {
        x: gx + 3.2,
        y: gy + 1.0,
        vx: 0.0,
        vy: -2.0,
        radius: 1.8,
        ttl: 20,
        from_player: true,
        overcharged: false,
        kind: entities::ProjectileKind::KineticBullet,
    });
    game.step();
    assert!(
        game.aliens[g_idx].hull.omega.abs() > 0.005,
        "Off-center kinetic projectile bullet must impart rotational torque (omega) to enemy hull"
    );

    // Verify 2.5D sub-cell (4x = 960x540) VGA renderer + side gauges run cleanly
    game.render_to_pixel_buffer();
    assert_eq!(
        game.pixel_buffer.len(),
        game.render_width() * game.render_height() * 4
    );

    // 4. Verify overlapping enemy ships bump off each other without overlapping
    game.aliens.truncate(2);
    game.aliens[0].hull.x = 110.0;
    game.aliens[0].hull.y = 45.0;
    game.aliens[0].hull.vx = 0.25;
    game.aliens[1].hull.x = 113.0;
    game.aliens[1].hull.y = 45.0;
    game.aliens[1].hull.vx = -0.25;
    let d_before = (game.aliens[1].hull.x - game.aliens[0].hull.x).abs();
    game.step();
    let d_after = (game.aliens[1].hull.x - game.aliens[0].hull.x).abs();
    assert!(
        d_after > d_before,
        "Overlapping enemy ships must bump apart on collision (d_before={}, d_after={})",
        d_before,
        d_after
    );

    // 5. Verify severely fractured enemy vessels with large cracks cutting through them
    //    have their weapons disabled and transition cleanly into the destroyed state
    let mut broken_gunship = entities::AlienVessel {
        role: AlienRole::TwinGunship,
        hull: dzog_core::CohesiveBody::new_twin_gunship(999, 120.0, 48.0, 6.8),
        fire_cooldown: 0,
        carrier_phase: 0.0,
        cubes_healed: 0,
        fracture_hits: 0,
        cleave_count: 0,
    };
    assert!(
        !broken_gunship.weapons_disabled() && !broken_gunship.is_critically_broken(false, 0),
        "Intact TwinGunship should be combat-operational"
    );

    // Carve a deep through-fracture across the central fuselage
    broken_gunship
        .hull
        .inflict_progressive_fracture(120.0, 48.0, 0.0, -1.0, 2.25, None);
    broken_gunship.fracture_hits = 2;
    let split_parts = broken_gunship
        .hull
        .split_connected_components(&mut game.next_body_id, None);
    assert!(
        broken_gunship.weapons_disabled(),
        "Severely fractured enemy ship must have its weapons disabled"
    );
    assert!(
        broken_gunship.is_critically_broken(true, split_parts.len()),
        "Enemy ship with a large fracture cutting through its fuselage must enter the killed state"
    );

    game.aliens.clear();
    game.bolts.clear();
    game.aliens.push(broken_gunship);
    game.step();
    assert!(
        game.aliens.is_empty(),
        "Critically broken enemy ship must be removed in the combat sweep rather than remaining alive"
    );
    assert!(
        game.bolts.iter().all(|b| b.from_player),
        "Critically broken enemy ship must never fire projectiles"
    );
}
