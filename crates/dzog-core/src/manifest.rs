//! Specification and Checkpoint serialization for shareable situations.

use serde::{Deserialize, Serialize};
use crate::boundary::{BoundaryMask, OuterBoundaryRule};
use crate::grid::{AcousticState, GridConfig};
use crate::material::MaterialField;
use crate::probe::PressureProbe;
use crate::solver::{AcousticSolver, SolverConfig};

/// Probe declaration inside a field specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeDef {
    pub id: String,
    pub x: usize,
    pub y: usize,
    pub kernel_radius: usize,
    pub event_threshold: Option<f32>,
}

/// Specification artifact: authored laws, materials, boundaries, inputs, and initial commitments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSpec {
    pub name: String,
    pub grid: GridConfig,
    pub outer_boundary: OuterBoundaryRule,
    pub dt: f32,
    pub cfl_safety: f32,
    pub substeps_per_macro: usize,
    pub default_density: f32,
    pub default_bulk_modulus: f32,
    pub default_damping: f32,
    pub probes: Vec<ProbeDef>,
}

impl FieldSpec {
    /// Serialize specification to a RON formatted string.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        let pretty = ron::ser::PrettyConfig::default();
        ron::ser::to_string_pretty(self, pretty)
    }

    /// Deserialize specification from a RON formatted string.
    pub fn from_ron(s: &str) -> Result<Self, ron::de::SpannedError> {
        ron::from_str(s)
    }

    /// Instantiate an initial AcousticSolver from this specification.
    pub fn instantiate_solver(&self) -> Result<AcousticSolver, String> {
        let state = AcousticState::new(self.grid);
        let material = MaterialField::homogeneous(
            self.grid,
            self.default_density,
            self.default_bulk_modulus,
            self.default_damping,
        );
        let boundary = BoundaryMask::new(self.grid, self.outer_boundary);
        let config = SolverConfig::new(self.dt, self.cfl_safety, self.substeps_per_macro);

        let mut solver = AcousticSolver::new(state, material, boundary, config)?;
        for p in &self.probes {
            let mut probe = PressureProbe::new(&p.id, p.x, p.y, p.kernel_radius);
            if let Some(t) = p.event_threshold {
                probe = probe.with_threshold(t);
            }
            solver.add_probe(probe);
        }
        Ok(solver)
    }
}

/// Checkpoint artifact: complete reproducible snapshot of simulation state at a step boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub step: usize,
    pub time: f32,
    pub state: AcousticState,
    pub material: MaterialField,
    pub boundary: BoundaryMask,
    pub config: SolverConfig,
}

impl Checkpoint {
    /// Capture complete checkpoint from an ongoing solver.
    pub fn capture(solver: &AcousticSolver) -> Self {
        Self {
            step: solver.current_step,
            time: solver.current_time,
            state: solver.state.clone(),
            material: solver.material.clone(),
            boundary: solver.boundary.clone(),
            config: solver.config,
        }
    }

    /// Restore solver from this checkpoint.
    pub fn restore(&self) -> Result<AcousticSolver, String> {
        let mut solver = AcousticSolver::new(
            self.state.clone(),
            self.material.clone(),
            self.boundary.clone(),
            self.config,
        )?;
        let count = solver.material.config.cell_count();
        if solver.state.luminescence.len() != count {
            solver.state.luminescence = vec![0.0; count];
        }
        if solver.material.cohesion.is_empty() {
            solver.material.cohesion = vec![0.0; count];
        }
        if solver.material.yield_pressure.is_empty() {
            solver.material.yield_pressure = vec![f32::INFINITY; count];
        }
        solver.current_step = self.step;
        solver.current_time = self.time;
        Ok(solver)
    }

    /// Serialize checkpoint to RON string.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        let pretty = ron::ser::PrettyConfig::default();
        ron::ser::to_string_pretty(self, pretty)
    }

    /// Deserialize checkpoint from RON string.
    pub fn from_ron(s: &str) -> Result<Self, ron::de::SpannedError> {
        ron::from_str(s)
    }
}
