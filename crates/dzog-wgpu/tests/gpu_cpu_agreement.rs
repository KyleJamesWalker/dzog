use dzog_core::*;
use dzog_wgpu::*;

#[test]
fn test_gate1_gpu_cpu_numerical_agreement() {
    let ctx = match pollster::block_on(GpuContext::init_headless()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Skipping GPU agreement test (no compatible GPU adapter): {e}");
            return;
        }
    };

    let grid = GridConfig::new(32, 32, 1.0, 1.0);
    let mut state = AcousticState::new(grid);
    let material = MaterialField::homogeneous(grid, 1.0, 1.0, 0.0);
    let boundary = BoundaryMask::new(grid, OuterBoundaryRule::RigidWall);
    let dt = SolverConfig::compute_cfl_dt(&grid, &material, 0.7);
    let config = SolverConfig::new(dt, 0.7, 1);

    // Inject a smooth Gaussian impulse at center
    let impulse = GaussianImpulse::new(16.0, 16.0, 2.5, 1.0);
    impulse.inject(&mut state);

    // 1. CPU reference simulation
    let mut cpu_solver = AcousticSolver::new(
        state.clone(),
        material.clone(),
        boundary.clone(),
        config,
    ).unwrap();

    let substeps = 15;
    for _ in 0..substeps {
        cpu_solver.step();
    }

    // 2. GPU simulation
    let gpu_sim = GpuSimulation::new(
        &ctx,
        &state,
        &material,
        &boundary,
        &config,
    ).expect("Failed to initialize GPU simulation");

    gpu_sim.step(&ctx, substeps);

    // Read back state from GPU
    let gpu_state = gpu_sim.readback_state(&ctx);

    // 3. Compare CPU and GPU states
    let mut max_p_diff = 0.0f32;
    let mut max_vx_diff = 0.0f32;
    let mut max_vy_diff = 0.0f32;

    for i in 0..grid.cell_count() {
        let p_diff = (cpu_solver.state.pressure[i] - gpu_state.pressure[i]).abs();
        let vx_diff = (cpu_solver.state.vx[i] - gpu_state.vx[i]).abs();
        let vy_diff = (cpu_solver.state.vy[i] - gpu_state.vy[i]).abs();

        if p_diff > max_p_diff { max_p_diff = p_diff; }
        if vx_diff > max_vx_diff { max_vx_diff = vx_diff; }
        if vy_diff > max_vy_diff { max_vy_diff = vy_diff; }
    }

    println!(
        "GPU vs CPU after {substeps} substeps: max_p_diff={max_p_diff:.6}, max_vx_diff={max_vx_diff:.6}, max_vy_diff={max_vy_diff:.6}"
    );

    // Floating-point agreement between WGSL compute shader and CPU solver should be within 1e-4
    assert!(
        max_p_diff < 1e-4,
        "CPU and GPU pressure should agree within numerical tolerance (got {max_p_diff})"
    );
    assert!(
        max_vx_diff < 1e-4,
        "CPU and GPU vx should agree within numerical tolerance (got {max_vx_diff})"
    );
    assert!(
        max_vy_diff < 1e-4,
        "CPU and GPU vy should agree within numerical tolerance (got {max_vy_diff})"
    );

    // 4. Verify GPU-native impulse injection without CPU readback
    gpu_sim.inject_impulse_gpu(&ctx, 16.0, 16.0, 2.5, 2.0, 1.0, 0.0);
    gpu_sim.step(&ctx, 4);
    let post_impulse_state = gpu_sim.readback_state(&ctx);
    let max_p_after = post_impulse_state.pressure.iter().copied().fold(0.0f32, f32::max);
    assert!(
        max_p_after > 0.5,
        "GPU-native impulse shader must excite pressure and velocity on the GPU lattice (got {max_p_after})"
    );
}
