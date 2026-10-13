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

#[test]
fn test_bohmian_quantum_potential_well_on_wavepacket() {
    let grid = GridConfig::new(40, 40, 1.0, 1.0);
    let mut state = AcousticState::new(grid);

    // Inject a smooth Gaussian wave packet centered at (20, 20)
    let impulse = GaussianImpulse::new(20.0, 20.0, 3.0, 2.0);
    impulse.inject(&mut state);

    // At the peak of a Gaussian envelope R(x,y), the curvature Laplacian(R) < 0,
    // so the Bohmian Quantum Potential Q = -Laplacian(R)/R > 0 at the peak,
    // while in the surrounding inflection ring Laplacian(R) > 0 => Q < 0.
    let q_center = state.quantum_potential_at(20, 20);
    let q_far = state.quantum_potential_at(5, 5);

    assert!(
        q_center > 0.1,
        "Bohmian quantum potential at Gaussian peak must reflect local envelope curvature (got {q_center})"
    );
    assert!(
        q_far.abs() < 1e-3,
        "Quantum potential far from excitation should vanish (got {q_far})"
    );

    // Verify quantum guidance force is non-zero on the flank of the wave packet
    let (fx, fy) = state.quantum_force_at(22, 20);
    assert!(
        fx.abs() > 0.01 && fy.abs() < 1e-5,
        "Quantum force on +X flank should be directed along X symmetry axis (fx={fx}, fy={fy})"
    );
}

#[test]
fn test_cohesion_erosion_under_acoustic_overpressure() {
    let grid = GridConfig::new(32, 16, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);

    // Paint a cohesive solid region at [14..18, 6..10] with yield_pressure = 0.8
    material.set_cohesive_region(14, 18, 6, 10, 1.2, 1.2, 0.005, 1.0, 0.8);
    let idx_target = grid.index(15, 8);
    assert_eq!(material.cohesion[idx_target], 1.0);

    // Inject high-amplitude pressure wave (p = 3.5 > 0.8 yield) over the region
    let blast = GaussianImpulse::new(15.0, 8.0, 2.5, 3.5);
    blast.inject(&mut state);

    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);
    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    for _ in 0..8 {
        solver.step();
    }

    let eroded_cohesion = solver.material.cohesion[idx_target];
    assert!(
        eroded_cohesion < 1.0,
        "Overpressure exceeding yield threshold must erode material cohesion (got {eroded_cohesion})"
    );
}
