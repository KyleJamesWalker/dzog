use dzog_core::*;

#[test]
fn test_gate1_zero_state_remains_zero() {
    let grid = GridConfig::new(32, 32, 1.0, 1.0);
    let state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    for _ in 0..100 {
        solver.step();
    }

    let energy = FieldEnergy::compute(&solver.state, &solver.material);
    assert_eq!(energy.total, 0.0, "Zero state must remain strictly zero");
}

#[test]
fn test_gate1_pulse_travel_time() {
    // 1D-like wave propagation in a uniform channel of length 60
    let grid = GridConfig::new(60, 10, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    // c = sqrt(K / rho) = sqrt(1 / 1) = 1.0 cell/time_unit
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);

    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    // Launch impulse at x = 10, y = 5
    let impulse = GaussianImpulse::new(10.0, 5.0, 2.0, 1.0);
    impulse.inject(&mut state);

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    // Probe placed at x = 30, y = 5 (distance = 20 cells)
    let probe = PressureProbe::new("sensor", 30, 5, 0);
    solver.add_probe(probe);

    // With wave speed c = 1.0 and distance d = 20 cells, theoretical arrival time is t = 20.0
    let mut peak_time = 0.0;
    let mut peak_val = 0.0f32;

    for _ in 0..70 {
        solver.step();
        let current_val = solver.probes[0].history.last().unwrap().pressure;
        if current_val > peak_val {
            peak_val = current_val;
            peak_time = solver.current_time;
        }
    }

    assert!(peak_val > 0.05, "Pulse should reach the probe");
    // Arrival peak should be within +/- 1.5 time units of t = 20.0
    assert!(
        (peak_time - 20.0).abs() <= 1.5,
        "Pulse peak arrival time ({peak_time}) should match theoretical travel time (20.0)"
    );
}

#[test]
fn test_gate1_lossless_energy_conservation() {
    let grid = GridConfig::new(40, 40, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0); // gamma = 0: lossless
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    let impulse = GaussianImpulse::new(20.0, 20.0, 3.0, 1.0);
    impulse.inject(&mut state);

    let initial_energy = FieldEnergy::compute(&state, &material);
    assert!(initial_energy.total > 0.0);

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    // Run for 150 substeps (multiple reflections off rigid walls)
    for _ in 0..150 {
        solver.step();
    }

    let final_energy = FieldEnergy::compute(&solver.state, &solver.material);
    let drift = final_energy.relative_drift(&initial_energy);

    // In a conservative staggered Yee scheme with rigid walls, energy drift is bounded
    assert!(
        drift < 0.08,
        "Lossless energy drift ({drift:.4}) must remain small and bounded"
    );
}

#[test]
fn test_gate2_impedance_reflection_coefficient() {
    // Test wave encountering an interface between Medium 1 (Z1) and Medium 2 (Z2)
    // Medium 1: rho = 1.0, K = 1.0 => c1 = 1.0, Z1 = 1.0
    // Medium 2: rho = 2.0, K = 2.0 => c2 = 1.0, Z2 = 2.0
    // Theoretical reflection coefficient R = (Z2 - Z1) / (Z2 + Z1) = (2 - 1) / (2 + 1) = 1/3 ~ 0.333
    let grid = GridConfig::new(80, 12, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let mut material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);

    // Set right half (x >= 40) to Medium 2
    material.set_region(40, 80, 0, 12, 2.0, 2.0, 0.0);

    let z1 = material.impedance_at(10, 5);
    let z2 = material.impedance_at(60, 5);
    let theoretical_r = (z2 - z1) / (z2 + z1);
    assert!((theoretical_r - 1.0 / 3.0).abs() < 1e-4);

    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    // Launch a planar 1D wave pulse across all y at x = 20
    let sigma = 3.0f32;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let z1 = material.impedance_at(20, 0);

    for y in 0..12 {
        for x in 10..30 {
            let dx = x as f32 - 20.0;
            let p_val = (-dx * dx / two_sigma_sq).exp();
            let idx = grid.index(x, y);
            state.pressure[idx] = p_val;
            state.vx[idx] = p_val / z1; // forward traveling plane wave
        }
    }

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    // Probe 1 monitors incident and reflected pulse at x = 25
    let probe = PressureProbe::new("reflector_probe", 25, 6, 0);
    solver.add_probe(probe);

    // Run until packet hits interface at x=40 and reflected packet returns to x=25
    for _ in 0..140 {
        solver.step();
    }

    let history = &solver.probes[0].history;
    // Incident peak occurs around t ~ 5 / c
    let incident_peak = history.iter().take(35).map(|s| s.pressure).fold(0.0f32, f32::max);
    // Reflected peak occurs around t ~ (15 + 15) / c = 30
    let reflected_peak = history.iter().skip(35).map(|s| s.pressure).fold(0.0f32, f32::max);

    let measured_r = reflected_peak / incident_peak;
    assert!(
        (measured_r - theoretical_r).abs() < 0.05,
        "Measured reflection coefficient ({measured_r:.3}) should match theoretical R ({theoretical_r:.3})"
    );
}

#[test]
fn test_gate3_offline_audio_export() {
    let grid = GridConfig::new(32, 32, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.05);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    let impulse = GaussianImpulse::new(16.0, 16.0, 2.0, 1.0);
    impulse.inject(&mut state);

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();
    let probe = PressureProbe::new("audio_listener", 10, 10, 1);
    solver.add_probe(probe);

    for _ in 0..100 {
        solver.step();
    }

    let export_cfg = AudioExportConfig {
        sample_rate: 48000,
        time_scale_a: 1.0,
        gain: 1.0,
    };

    let audio = resample_probe_to_audio(&solver.probes[0], &export_cfg);
    assert!(!audio.is_empty(), "Resampled audio should contain samples");

    // Test writing to a temporary WAV file
    let tmp_path = std::env::temp_dir().join("test_gate3_probe.wav");
    write_stereo_wav(&tmp_path, 48000, &audio, &audio).unwrap();
    assert!(tmp_path.exists());
    let _ = std::fs::remove_file(tmp_path);
}

#[test]
fn test_gate5_checkpoint_roundtrip() {
    let grid = GridConfig::new(32, 32, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = 0.4;
    let config = SolverConfig::new(dt, 0.7, 1);

    let impulse = GaussianImpulse::new(16.0, 16.0, 2.5, 1.0);
    impulse.inject(&mut state);

    let mut solver = AcousticSolver::new(state, material, boundary, config).unwrap();

    // Advance 25 steps
    for _ in 0..25 {
        solver.step();
    }

    // Capture checkpoint and serialize to RON
    let checkpoint = Checkpoint::capture(&solver);
    let ron_str = checkpoint.to_ron().expect("Failed to serialize checkpoint to RON");

    // Deserialize checkpoint
    let restored_checkpoint = Checkpoint::from_ron(&ron_str).expect("Failed to deserialize checkpoint");
    let mut restored_solver = restored_checkpoint.restore().unwrap();

    // Run both solvers for 25 more steps
    for _ in 0..25 {
        solver.step();
        restored_solver.step();
    }

    // Compare states: must match exactly
    assert_eq!(solver.current_step, restored_solver.current_step);
    for i in 0..grid.cell_count() {
        assert_eq!(
            solver.state.pressure[i],
            restored_solver.state.pressure[i],
            "Pressure mismatch at index {i}"
        );
        assert_eq!(
            solver.state.vx[i],
            restored_solver.state.vx[i],
            "vx mismatch at index {i}"
        );
        assert_eq!(
            solver.state.vy[i],
            restored_solver.state.vy[i],
            "vy mismatch at index {i}"
        );
    }
}
