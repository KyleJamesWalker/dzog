use dzog_core::*;

#[test]
fn test_moving_boundary_piston_acoustic_radiation() {
    // In acoustics, a boundary moving into a stationary fluid with normal velocity V_w
    // radiates a wave with pressure perturbation Delta p = Z * V_w (piston radiation).
    let grid = GridConfig::new(60, 10, 1.0, 1.0);
    let state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0); // Z = 1.0
    let mut boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);

    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    // Set a moving vertical boundary (piston/paddle) at x = 10 moving rightward with vx = 0.5
    let v_piston = 0.5f32;
    for y in 0..10 {
        let idx = grid.index(10, y);
        boundary.wall_east[idx] = true;
        boundary.wall_vx[idx] = v_piston;
    }

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    // Probe at x = 20 (distance = 10 from piston)
    let probe = PressureProbe::new("downstream_sensor", 20, 5, 0);
    solver.add_probe(probe);

    // Run for 35 substeps
    for _ in 0..35 {
        solver.step();
    }

    let last_sample = solver.probes[0].history.last().unwrap().pressure;
    // Acoustic impedance Z = 1.0. The radiated pressure wave front should be approximately Z * V_piston = 0.5
    assert!(
        last_sample > 0.35 && last_sample < 0.65,
        "Radiated pressure ({last_sample:.3}) should match acoustic impedance piston radiation (Z * V = 0.5)"
    );
}

#[test]
fn test_moving_paddle_momentum_transfer() {
    // A wave packet reflecting off a stationary wall vs a paddle moving towards it.
    // Moving into the wave packet does boundary work, increasing reflected amplitude and energy.
    let grid = GridConfig::new(50, 10, 1.0, 1.0);
    let dt = 0.45;
    let config = SolverConfig::new(dt, 0.7, 1);

    // Case 1: Stationary paddle at x = 35
    let mut state1 = AcousticState::new(grid);
    let material1 = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let mut boundary1 = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    for y in 0..10 {
        let idx = grid.index(35, y);
        boundary1.wall_east[idx] = true;
        boundary1.wall_vx[idx] = 0.0;
    }
    // Launch wave towards wall
    for y in 0..10 {
        for x in 15..25 {
            let dx = x as f32 - 20.0;
            let val = (-dx * dx / 8.0).exp();
            let idx = grid.index(x, y);
            state1.pressure[idx] = val;
            state1.vx[idx] = val;
        }
    }
    let mut solver_stationary = AcousticSolver::new(state1, material1, boundary1, config).unwrap();

    // Case 2: Moving paddle moving leftward into the wave (vx = -0.3)
    let mut state2 = AcousticState::new(grid);
    let material2 = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let mut boundary2 = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    for y in 0..10 {
        let idx = grid.index(35, y);
        boundary2.wall_east[idx] = true;
        boundary2.wall_vx[idx] = -0.3;
    }
    for y in 0..10 {
        for x in 15..25 {
            let dx = x as f32 - 20.0;
            let val = (-dx * dx / 8.0).exp();
            let idx = grid.index(x, y);
            state2.pressure[idx] = val;
            state2.vx[idx] = val;
        }
    }
    let mut solver_moving = AcousticSolver::new(state2, material2, boundary2, config).unwrap();

    // Run both solvers through the collision
    for _ in 0..60 {
        solver_stationary.step();
        solver_moving.step();
    }

    let energy_stationary = FieldEnergy::compute(&solver_stationary.state, &solver_stationary.material);
    let energy_moving = FieldEnergy::compute(&solver_moving.state, &solver_moving.material);

    // Moving boundary into the wave does mechanical work on the wave packet
    assert!(
        energy_moving.total > energy_stationary.total,
        "Paddle moving towards wave must increase wave energy (E_moving={:.3} > E_stat={:.3})",
        energy_moving.total, energy_stationary.total
    );
}
