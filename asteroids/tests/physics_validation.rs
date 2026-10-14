use dzog_core::*;

#[path = "../src/field_game.rs"]
mod field_game;

use field_game::*;

#[test]
fn test_soliton_bolt_topological_cleaving_of_asteroid() {
    let mut game = AsteroidsFieldGame::new(128, 96);
    game.asteroids.clear();
    game.aliens.clear();

    // Place a PiezoQuartz asteroid directly in front of the player ship at (64, 30)
    let rock = CohesiveBody::new_asteroid(1, 64.0, 30.0, 6.5, 6, 101, MineralClass::PiezoQuartz);
    game.asteroids.push(rock);

    game.ship_x = 64.0;
    game.ship_y = 48.0;
    game.ship_theta = -std::f32::consts::FRAC_PI_2; // Aiming straight North (-Y)
    game.capacitor = 100.0;

    // Fire an Overcharged Phase-Lance bolt straight into the asteroid
    assert!(game.fire_soliton_bolt(true));

    let mut cleaved = false;
    for _ in 0..25 {
        let ev = game.step();
        if matches!(ev, AsteroidEvent::AsteroidCleaved { .. }) {
            cleaved = true;
            break;
        }
    }

    assert!(
        cleaved && game.asteroids.len() >= 2,
        "Overcharged soliton bolt must cleave asteroid into >= 2 tumbling child fragments (count={})",
        game.asteroids.len()
    );
    assert!(game.topological_cleaves >= 1);
}

#[test]
fn test_two_hit_resonance_crit_cleaving() {
    let mut game = AsteroidsFieldGame::new(128, 96);
    game.asteroids.clear();
    game.aliens.clear();

    let rock = CohesiveBody::new_asteroid(1, 64.0, 32.0, 6.8, 7, 77, MineralClass::BismuthCrystal);
    game.asteroids.push(rock);

    game.ship_x = 64.0;
    game.ship_y = 48.0;
    game.ship_theta = -std::f32::consts::FRAC_PI_2;
    game.capacitor = 0.0;

    // Hit 1: Standard bolt carves internal waveguide fissures and imparts spin
    game.fire_soliton_bolt(false);
    let mut cracked = false;
    for _ in 0..20 {
        if matches!(game.step(), AsteroidEvent::AsteroidCracked { .. }) {
            cracked = true;
            break;
        }
    }
    assert!(cracked, "First standard bolt should crack the asteroid crystal");
    assert_eq!(game.asteroids.len(), 1, "First standard hit should not sever intact crystal yet");

    // Hit 2: Second standard bolt strikes the cracked crystal, triggering a Resonance Crit cleave!
    game.fire_soliton_bolt(false);
    let mut cleaved = false;
    for _ in 0..25 {
        if matches!(game.step(), AsteroidEvent::AsteroidCleaved { .. }) {
            cleaved = true;
            break;
        }
    }
    assert!(
        cleaved && game.asteroids.len() >= 2,
        "Second strike into cracked crystal must trigger Resonance Crit topological split"
    );
}

#[test]
fn test_alien_bohmian_saucer_combat_and_shield_parry() {
    let mut game = AsteroidsFieldGame::new(128, 96);
    game.asteroids.clear();
    game.aliens.clear();

    // Spawn a Bohmian Saucer at (64, 28)
    let mut saucer_hull = CohesiveBody::new_saucer(10, 64.0, 28.0, 4.8);
    saucer_hull.vx = 0.0;
    saucer_hull.vy = 0.0;
    game.aliens.push(AlienEnemy {
        kind: AlienKind::BohmianSaucer,
        hull: saucer_hull,
        fire_cooldown: 1,
        telegraph_timer: 0,
        aim_angle: std::f32::consts::FRAC_PI_2,
    });

    game.ship_x = 64.0;
    game.ship_y = 52.0;
    game.ship_theta = -std::f32::consts::FRAC_PI_2;
    game.capacitor = 20.0;

    // Step until the Bohmian Saucer telegraphs and fires an alien soliton bolt toward the player
    let mut alien_bolt_spawned = false;
    for _ in 0..30 {
        game.step();
        if game.bolts.iter().any(|b| !b.from_player) {
            alien_bolt_spawned = true;
            break;
        }
    }
    assert!(
        alien_bolt_spawned,
        "Bohmian Saucer must telegraph and fire a directional soliton bolt at the player"
    );

    // Move alien bolt into the player's shield ring and parry it with the Antiphase Interference Shield!
    game.bolts[0].x = game.ship_x;
    game.bolts[0].y = game.ship_y - 4.0;
    let cap_before = game.capacitor;
    let harvested = game.trigger_antiphase_shield();
    assert!(
        harvested > 1.0 && game.capacitor > cap_before && game.shield_parries == 1,
        "Antiphase shield must annihilate incoming alien bolt and harvest energy into ship capacitor"
    );

    // Now fire player bolt back at the Bohmian Saucer to destroy and cleave it!
    game.aliens[0].hull.x = 64.0;
    game.aliens[0].hull.y = 38.0;
    game.capacitor = 100.0;
    game.fire_soliton_bolt(true);

    let mut alien_destroyed = false;
    for _ in 0..25 {
        if matches!(game.step(), AsteroidEvent::AlienDestroyed { .. }) {
            alien_destroyed = true;
            break;
        }
    }
    assert!(
        alien_destroyed && game.aliens.is_empty(),
        "Player soliton bolt must cleave and destroy the alien Bohmian Saucer"
    );
}

#[test]
fn test_thruster_backblast_and_field_brake_and_modes() {
    let mut game = AsteroidsFieldGame::new(128, 96);
    game.asteroids.clear();
    game.aliens.clear();

    // Place a small asteroid shard directly behind the ship's exhaust nozzle
    let shard = CohesiveBody::new_asteroid(1, 56.0, 48.0, 4.0, 6, 12, MineralClass::PiezoQuartz);
    game.asteroids.push(shard);

    game.ship_x = 64.0;
    game.ship_y = 48.0;
    game.ship_theta = 0.0; // Facing East (+X), exhaust points West (-X) toward shard at (56, 48)

    for _ in 0..6 {
        game.thrust_forward(1.2);
        game.step();
    }

    assert!(
        game.ship_vx > 0.05,
        "Main reaction thruster must accelerate ship forward (+X)"
    );
    assert!(
        game.asteroids[0].vx < -0.001,
        "Reaction thruster exhaust plume must physically push trailing asteroid shard backward (-X, got vx={})",
        game.asteroids[0].vx
    );

    // Engage Field-Brake bow shock
    let speed_before = game.ship_vx;
    game.apply_field_brake();
    assert!(
        game.ship_vx < speed_before * 0.75,
        "Field-Brake must bleed ship momentum into a forward acoustic bow-shock"
    );

    // Test topology mode toggle (ToroidalPeriodic <-> GravityWellArena), view modes, and branching
    game.save_branch_checkpoint();
    game.toggle_topology_mode();
    assert_eq!(game.topology, WorldTopologyMode::GravityWellArena);
    game.step();
    game.cycle_view_mode();
    game.render_to_pixel_buffer();
    assert!(game.restore_branch_checkpoint());
    assert_eq!(game.topology, WorldTopologyMode::ToroidalPeriodic);

    // Test GPU-accelerated stepping and Autopilot mode
    game.autopilot_enabled = true;
    game.step();
    game.autopilot_enabled = false;

    // Verify that moving/thrusting/rotating the ship in open space generates ZERO sound
    game.asteroids.clear();
    game.aliens.clear();
    game.bolts.clear();
    game.solver.state.clear();
    if let Ok(mut g) = game.shared_audio_frame.lock() {
        *g = FieldAcousticFrame::default();
    }
    game.thrust_forward(1.0);
    game.rotate_ship(1.0);
    game.step();
    let mut synth = FieldModalSynthesizer::new(game.shared_audio_frame.clone());
    assert_eq!(
        synth.next_stereo_frame(),
        (0.0, 0.0),
        "Moving/thrusting in open space must produce exact 0.0 silence"
    );

    // Verify player laser firing produces immediate, audible stereo laser sweep
    game.fire_soliton_bolt(false);
    let mut peak_laser = 0.0f32;
    for _ in 0..64 {
        let (l, r) = synth.next_stereo_frame();
        peak_laser = peak_laser.max(l.abs()).max(r.abs());
    }
    assert!(
        peak_laser > 0.25,
        "Firing player soliton bolt must produce crisp audible laser sound (got {peak_laser})"
    );

    // Verify asteroid break produces a loud, unmistakable cavitation explosion (> 0.45 peak)
    if let Ok(mut g) = game.shared_audio_frame.lock() {
        g.trigger_cavitation_break(0.0, 1.5);
    }
    let mut peak_break = 0.0f32;
    for _ in 0..128 {
        let (l, r) = synth.next_stereo_frame();
        peak_break = peak_break.max(l.abs()).max(r.abs());
    }
    assert!(
        peak_break > 0.45,
        "Asteroid cleave/shatter must produce a loud cavitation explosion (got {peak_break})"
    );
}
