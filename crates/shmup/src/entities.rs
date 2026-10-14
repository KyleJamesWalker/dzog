//! Field-coupled actor archetypes for Crystalline Corridor (`crates/shmup`).
//!
//! Operationalizes `philosophy.md`:
//! - No integer hitpoint counters (`hp: 3`). The player ship is a situated 3-lobe crystal hull
//!   (Port Wing, Central Core, Starboard Wing) whose structural lobes cleave under impact and
//!   re-crystallize via exciton annealing.
//! - Alien formations are not spline-scripted; `ChladniSwarmMatriarch` cruisers radiate a
//!   standing-wave interference lattice while `BohmianSwarmDrone`s surf the resulting `-grad Q`
//!   quantum potential pockets.
//! - `PowerCube`s are spinning `PiezoQuartz` micro-condensates guided by `-grad Q` and `-grad p`
//!   that heal whichever damaged hull (player or alien) captures them.

use dzog_core::{Checkpoint, CohesiveBody, MineralClass};

/// Coherent plasma-soliton projectile coupled to the wavefield.
#[derive(Debug, Clone, PartialEq)]
pub struct SolitonBolt {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub radius: f32,
    pub ttl: u16,
    pub from_player: bool,
    pub overcharged: bool,
}

/// Alien antagonist archetype coupled to the PDE field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlienRole {
    /// Emits a Chladni standing-wave carrier lattice (`ChladniCarrierEmitter`) that organizes
    /// surrounding swarm drones into `-grad Q` nodal pockets.
    ChladniMatriarch,
    /// Lightweight resonator drone that surfs Bohmian quantum potential minima (`-grad Q`)
    /// around a Matriarch's carrier wave, scattering naturally when the wave is disrupted.
    BohmianSwarmDrone {
        matriarch_id: u32,
        nodal_slot: u8,
    },
    /// Independent interceptor that hunts Annealing Power Cubes when its hull is fractured.
    AnnealingHunter,
}

/// Field-coupled alien vessel.
#[derive(Debug, Clone, PartialEq)]
pub struct AlienVessel {
    pub role: AlienRole,
    pub hull: CohesiveBody,
    pub fire_cooldown: u16,
    pub carrier_phase: f32,
    pub cubes_healed: u32,
}

/// Annealing Piezo-Condensate ("Power Cube").
///
/// Released when a crystalline asteroid or alien hull shatters. Guided by radiation pressure
/// `(-grad p)` and Bohmian quantum potential curvature `(-grad Q)`.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerCube {
    pub body: CohesiveBody,
    pub ttl: u16,
    pub annealing_energy: f32,
}

/// High-level ecological event emitted during a simulation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShmupEvent {
    None,
    AsteroidCracked { body_id: u32, class: MineralClass },
    AsteroidCleaved { parent_id: u32, children_spawned: usize },
    AsteroidShattered { body_id: u32, cubes_spawned: usize },
    AlienDestroyed { role: AlienRole, cubes_spawned: usize },
    PlayerCubeAnnealed { intact_lobes: u32, healed_cells: usize },
    AlienCubeAnnealed { alien_id: u32, healed_cells: usize },
    PlayerLobeCleaved { remaining_lobes: u32 },
    ShieldParry,
}

/// Complete snapshot of a Crystalline Corridor situation for counterfactual branching (`[B]`/`[U]`).
#[derive(Debug, Clone)]
pub struct ShmupBranchSnapshot {
    pub checkpoint: Checkpoint,
    pub player_hull: CohesiveBody,
    pub capacitor: f32,
    pub asteroids: Vec<CohesiveBody>,
    pub aliens: Vec<AlienVessel>,
    pub power_cubes: Vec<PowerCube>,
    pub bolts: Vec<SolitonBolt>,
    pub canyon_scroll: f32,
    pub score: u32,
    pub wave_number: u32,
}
