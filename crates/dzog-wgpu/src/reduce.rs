//! CPU mirror of `reduce_body.wgsl` for targets without a blocking GPU readback (wasm32).

use dzog_core::AcousticState;
use crate::simulation::{GpuBodyQuery, GpuBodyReduction, MAX_GPU_BODY_QUERIES};

/// Same force, torque, and peak-pressure reduction as `GpuSimulation::reduce_bodies_gpu`.
///
/// `q_pressure` is the pressure before the last substep's pressure update: the GPU stores Q
/// during the velocity pass, so it lags `state.pressure` by one half-step.
pub fn reduce_bodies_cpu(
    state: &AcousticState,
    q_pressure: &[f32],
    periodic: bool,
    queries: &[GpuBodyQuery],
) -> Vec<GpuBodyReduction> {
    let w = state.config.width as i32;
    let h = state.config.height as i32;
    let inv_dx = 1.0 / state.config.dx;
    let inv_dy = 1.0 / state.config.dy;
    let wrap = |v: i32, n: i32| (((v % n) + n) % n) as usize;
    let p = |x: usize, y: usize| state.pressure[state.config.index(x, y)];
    let q = |x: usize, y: usize| {
        let (wu, hu) = (w as usize, h as usize);
        if !periodic && (x == 0 || x + 1 >= wu || y == 0 || y + 1 >= hu) {
            return 0.0;
        }
        let r = |x: usize, y: usize| {
            let p = q_pressure[state.config.index(x, y)];
            (p * p + 0.0064).sqrt()
        };
        let r_c = r(x, y);
        let lap_r = (r((x + 1) % wu, y) - 2.0 * r_c + r((x + wu - 1) % wu, y)) * inv_dx * inv_dx
            + (r(x, (y + 1) % hu) - 2.0 * r_c + r(x, (y + hu - 1) % hu)) * inv_dy * inv_dy;
        -lap_r / r_c
    };

    queries
        .iter()
        .take(MAX_GPU_BODY_QUERIES)
        .map(|query| {
            if query.radius <= 0.01 {
                let px = wrap(query.x.round() as i32, w);
                let py = wrap(query.y.round() as i32, h);
                let idx = state.config.index(px, py);
                return GpuBodyReduction {
                    fx: state.vx[idx],
                    fy: state.vy[idx],
                    tau_z: q(px, py),
                    max_pressure: state.pressure[idx],
                };
            }

            let r_cells = (query.radius.ceil() as i32 + 1).min(14);
            let r_sq_max = (query.radius + 1.0) * (query.radius + 1.0);
            let cx = query.x.round() as i32;
            let cy = query.y.round() as i32;
            let mut out = GpuBodyReduction::default();

            for oy in -r_cells..=r_cells {
                for ox in -r_cells..=r_cells {
                    let rx = (cx + ox) as f32 - query.x;
                    let ry = (cy + oy) as f32 - query.y;
                    if rx * rx + ry * ry > r_sq_max {
                        continue;
                    }

                    let gx = wrap(cx + ox, w);
                    let gy = wrap(cy + oy, h);
                    let xe = wrap(gx as i32 + 1, w);
                    let xw = wrap(gx as i32 - 1, w);
                    let yn = wrap(gy as i32 + 1, h);
                    let ys = wrap(gy as i32 - 1, h);

                    out.max_pressure = out.max_pressure.max(p(gx, gy).abs());

                    let grad_px = (p(xe, gy) - p(xw, gy)) * 0.5 * inv_dx;
                    let grad_py = (p(gx, yn) - p(gx, ys)) * 0.5 * inv_dy;
                    if grad_px * grad_px + grad_py * grad_py > 0.04 {
                        let q_fx = -(q(xe, gy) - q(xw, gy)) * 0.5 * inv_dx;
                        let q_fy = -(q(gx, yn) - q(gx, ys)) * 0.5 * inv_dy;
                        let dfx = -grad_px * 0.18 + q_fx * 0.025;
                        let dfy = -grad_py * 0.18 + q_fy * 0.025;
                        out.fx += dfx;
                        out.fy += dfy;
                        out.tau_z += rx * dfy - ry * dfx;
                    }
                }
            }
            out
        })
        .collect()
}
