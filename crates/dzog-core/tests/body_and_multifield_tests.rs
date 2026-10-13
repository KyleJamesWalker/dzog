use std::f32::consts::PI;
use dzog_core::*;

#[test]
fn test_rotating_cohesive_body_torque_and_topological_cleaving() {
    let grid = GridConfig::new(64, 64, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.01);
    let mut boundary = BoundaryMask::new(grid, OuterBoundaryRule::Periodic);

    // 1. Spawn a faceted PiezoQuartz asteroid at (32, 32)
    let mut rock = CohesiveBody::new_asteroid(1, 32.0, 32.0, 6.5, 7, 42, MineralClass::PiezoQuartz);
    let initial_cells = rock.active_cell_count();
    assert!(initial_cells > 40, "Asteroid should have > 40 active local crystal cells");

    // 2. Apply an off-center glancing impulse at top edge (32.0, 27.0) pushing +X
    rock.apply_contact_impulse(32.0, 27.0, 2.5, 0.0, Some((64.0, 64.0)));
    assert!(rock.vx > 0.01, "Off-center impulse must impart linear velocity +vx");
    assert!(
        rock.omega.abs() > 0.001,
        "Off-center impulse must impart angular velocity omega (got {})",
        rock.omega
    );

    // 3. Rotate the asteroid and rasterize into the periodic Yee lattice
    rock.theta = 0.65;
    rock.rasterize_to_lattice(&mut material, &mut boundary, true);
    let center_idx = grid.index(32, 32);
    assert!(
        (material.density[center_idx] - rock.material.rho).abs() < 1e-5,
        "Rotated asteroid must stamp its mineral density into the lattice"
    );

    // 4. First standard bolt strike carves a partial waveguide fissure without severing yet
    let crit1 = rock.carve_cleavage_fissure(28.0, 32.0, 1.0, 0.0, 1.0, Some((64.0, 64.0)));
    assert!(!crit1, "First standard strike on pristine crystal should carve waveguide fissures");
    assert!(
        rock.mean_cohesion() < 0.96,
        "First strike must reduce mean crystal cohesion"
    );

    // 5. Second strike along the cracked vein triggers a Resonance Crit and cleaves the topology!
    let crit2 = rock.carve_cleavage_fissure(28.5, 32.0, 1.0, 0.0, 1.2, Some((64.0, 64.0)));
    assert!(crit2, "Second strike into cracked vein must trigger a Resonance Crit full cleave");

    let mut next_id = 10;
    let children = rock.split_connected_components(&mut next_id, Some((64.0, 64.0)));
    assert!(
        children.len() >= 2,
        "Topological BFS must split cleaved asteroid into >= 2 child fragments (got {})",
        children.len()
    );

    // Verify child fragments diverge with opposite separation velocities
    let c0 = &children[0];
    let c1 = &children[1];
    let rel_vx = c0.vx - c1.vx;
    let rel_vy = c0.vy - c1.vy;
    assert!(
        rel_vx * rel_vx + rel_vy * rel_vy > 0.005,
        "Child fragments must separate with opposite cavitation momentum"
    );

    // Verify field force integration on rotated contour
    let impulse = GaussianImpulse::new(25.0, 32.0, 2.0, 3.0);
    impulse.inject_wrapped(&mut state, true);
    let (fx, _fy, _tau) = rock.integrate_field_forces(&state, 0.25, true);
    assert!(
        fx > 0.0,
        "High-pressure blast on West side of asteroid must push it East (+fx, got {fx})"
    );
}

#[test]
fn test_antiphase_shield_destructive_interference_and_thruster_plume() {
    let grid = GridConfig::new(48, 48, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);

    // 1. Inject an incoming enemy directional wave packet near (24, 20)
    let mut bolt = DirectionalWavePacket::new(24.0, 20.0, 2.0, 2.8, PI * 0.5);
    bolt.inject_wrapped(&mut state, &material, true);

    let energy_before = FieldEnergy::compute(&state, &material).total;
    assert!(energy_before > 1.0, "Incoming wave packet must carry positive Hamiltonian energy");

    // 2. Trigger Antiphase Shield Pulse centered at ship (24, 24) covering radius [1.5, 7.5]
    let shield = AntiphaseShieldPulse::new(24.0, 24.0, 1.5, 7.5, 0.88);
    let harvested = shield.cancel_and_harvest(&mut state, &material, true);
    let energy_after = FieldEnergy::compute(&state, &material).total;

    assert!(
        harvested > 0.8 * energy_before,
        "Antiphase shield must harvest > 80% of incident wave energy (before={energy_before}, harvested={harvested})"
    );
    assert!(
        energy_after < 0.2 * energy_before,
        "Destructive interference must reduce remaining wave energy by > 80% (after={energy_after})"
    );

    // 3. Test Reaction Thruster Plume momentum conservation
    state.clear();
    let plume = ReactionThrusterPlume::new(24.0, 24.0, 1.8, 2.0, PI); // Exhaust pointing West (-X)
    let (recoil_fx, recoil_fy) = plume.inject(&mut state, &material, true);
    assert!(
        recoil_fx > 1.0 && recoil_fy.abs() < 1e-4,
        "Westward exhaust plume must impart Eastward (+X) recoil thrust to the ship"
    );
    let center_vx = state.vx[grid.index(24, 24)];
    assert!(
        center_vx < -0.5,
        "Westward exhaust plume must inject negative particle velocity vx into the lattice (got {center_vx})"
    );
}

#[test]
fn test_poisson_gravity_well_and_binaural_doppler_sonification() {
    let grid = GridConfig::new(48, 48, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);

    // 1. Gravity well relaxation & orbital pull
    let mut gravity = GravityField::new(grid, 0.25);
    gravity.wells.push(GravityWell {
        x: 24.0,
        y: 24.0,
        strength: 4.0,
        core_radius: 3.0,
    });
    gravity.relax_poisson(&material, 8, true);

    let (gx, gy) = gravity.acceleration_at(16.0, 24.0, true);
    assert!(
        gx > 0.02 && gy.abs() < 1e-4,
        "Point West of gravity well must accelerate Eastward toward well center (got gx={gx}, gy={gy})"
    );

    // 2. Binaural cockpit sonifier Doppler frequency shift test
    // Create a standing spatial wave pattern along X: p(x, y) = sin(k * x)
    for y in 0..48 {
        for x in 0..48 {
            let idx = grid.index(x, y);
            state.pressure[idx] = (x as f32 * 0.85).sin() * 1.5;
        }
    }

    let mut slow_sonifier = BinauralCockpitSonifier::new(2.0, 256);
    let mut fast_sonifier = BinauralCockpitSonifier::new(2.0, 256);

    for step in 0..64 {
        slow_sonifier.sample_cockpit(&state, 4.0 + step as f32 * 0.25, 24.0, 0.0, true);
        fast_sonifier.sample_cockpit(&state, 4.0 + step as f32 * 0.85, 24.0, 0.0, true);
    }

    assert!(
        fast_sonifier.doppler_freq > slow_sonifier.doppler_freq,
        "Fast-moving ship crossing a wave train must measure a higher Doppler frequency ({}) than slow ship ({})",
        fast_sonifier.doppler_freq,
        slow_sonifier.doppler_freq
    );
}
