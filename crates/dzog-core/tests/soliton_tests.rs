use dzog_core::*;

#[test]
fn test_soliton_cohesion_vs_linear_dispersion() {
    let grid = GridConfig::new(80, 20, 1.0, 1.0);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);

    // Case 1: Pure linear acoustics (beta = 0.0)
    let config_linear = SolverConfig::new(dt, 0.7, 1);
    let mut state_linear = AcousticState::new(grid);
    let mut packet1 = DirectionalWavePacket::new(15.0, 10.0, 2.5, 1.5, 0.0);
    packet1.inject(&mut state_linear, &material);
    let mut solver_linear = AcousticSolver::new(state_linear, material.clone(), boundary.clone(), config_linear).unwrap();

    // Case 2: Nonlinear solitary field (beta = 0.8, alpha = 0.5)
    let nl = NonlinearLaw { beta: 0.8, alpha: 0.5, threshold_decay: 0.05 };
    let config_soliton = SolverConfig::new(dt, 0.7, 1).with_nonlinearity(nl);
    let mut state_soliton = AcousticState::new(grid);
    let mut packet2 = DirectionalWavePacket::new(15.0, 10.0, 2.5, 1.5, 0.0);
    packet2.inject(&mut state_soliton, &material);
    let mut solver_soliton = AcousticSolver::new(state_soliton, material, boundary, config_soliton).unwrap();

    // Advance both solvers for 60 steps as the packets propagate towards x = 50
    for _ in 0..60 {
        solver_linear.step();
        solver_soliton.step();
    }

    // Measure peak amplitude of both packets
    let max_p_linear = solver_linear.state.pressure.iter().copied().fold(0.0f32, f32::max);
    let max_p_soliton = solver_soliton.state.pressure.iter().copied().fold(0.0f32, f32::max);

    println!("After 60 steps: Linear peak = {max_p_linear:.3}, Soliton peak = {max_p_soliton:.3}");

    // The nonlinear solitary self-focusing maintains peak concentration and counteracts 2D dispersion
    assert!(
        max_p_soliton > max_p_linear,
        "Soliton field must preserve cohesive peak amplitude higher than dispersive linear wave (soliton={max_p_soliton:.3} > linear={max_p_linear:.3})"
    );
}

#[test]
fn test_soliton_wall_reflection_rebound() {
    let grid = GridConfig::new(50, 16, 1.0, 1.0);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);

    let nl = NonlinearLaw { beta: 0.6, alpha: 0.4, threshold_decay: 0.02 };
    let config = SolverConfig::new(dt, 0.7, 1).with_nonlinearity(nl);
    let mut state = AcousticState::new(grid);

    // Launch soliton towards the right wall at x = 49
    let mut packet = DirectionalWavePacket::new(30.0, 8.0, 2.5, 1.4, 0.0);
    packet.inject(&mut state, &material);

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    // Monitor probe at x = 35
    let probe = PressureProbe::new("rebound_sensor", 35, 8, 0);
    solver.add_probe(probe);

    // 1. Packet passes probe traveling rightwards (~10 steps)
    // 2. Hits right wall at x=49 and reflects leftwards (~30 steps)
    // 3. Passes probe traveling leftwards (~50 steps)
    for _ in 0..80 {
        solver.step();
    }

    let history = &solver.probes[0].history;
    let incident_peak = history.iter().take(25).map(|s| s.pressure).fold(0.0f32, f32::max);
    let reflected_peak = history.iter().skip(25).map(|s| s.pressure).fold(0.0f32, f32::max);

    assert!(incident_peak > 0.5, "Incident soliton should pass probe");
    assert!(reflected_peak > 0.5, "Reflected soliton should pass probe on return rebound");
}
