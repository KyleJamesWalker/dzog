//! Field-coupled sky and ground actor archetypes for *Shmup* (`crates/shmup` — Raptor 2.0).
//!
//! Operationalizes `philosophy.md`:
//! - **Player Hull**: Situated 3-lobe crystal ship (Port Wing, Central Core, Starboard Wing) whose
//!   structural lobes cleave under impact and re-crystallize via exciton annealing.
//! - **Medium/Large Enemy Progressive Fracturing**: `TwinGunship` strike craft, `ChladniMatriarch`
//!   cruisers, and ground domes accumulate internal fracture waveguides (`0.15 <= C < 0.55`).
//!   Repeated strikes into existing fractures widen and branch the fissures until wings, nacelles,
//!   or domes topologically cleave via BFS (`split_connected_components`).
//! - **Dual-Layer Ground & Sky Ecology**: Destructible `GeodesicFuelDome`s, `BiodomeReactor`s,
//!   `FlakTurret`s, and `CanalPatrolBoat`s couple to waveguide pipelines and water canals.

use dzog_core::{Checkpoint, CohesiveBody, MineralClass};

/// Ordnance classification for field-coupled projectiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileKind {
    /// High-velocity kinetic-soliton bullet that imparts strong linear & angular recoil (`r x J`)
    /// and progressive fractures to airborne hulls and asteroids.
    KineticBullet,
    /// Alt-fire Air-to-Ground finned missile (`ss_345bbfc`) that flies beneath sky squadrons and
    /// locks onto ground structures (`GeodesicFuelDome`, `BiodomeReactor`, `FlakTurret`, `CanalPatrolBoat`),
    /// delivering a seismic shockwave and pipeline chain-detonation.
    AirToGroundMissile,
    /// Enemy plasma-soliton orb.
    EnemyOrb,
}

/// Coherent projectile coupled to the wavefield.
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
    pub kind: ProjectileKind,
}

/// Airborne antagonist archetype coupled to the PDE field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlienRole {
    /// Emits a Chladni standing-wave carrier lattice (`ChladniCarrierEmitter`) that organizes
    /// surrounding swarm drones into `-grad Q` nodal pockets.
    ChladniMatriarch,
    /// Heavy twin-nacelle strike gunship (`ss_3ebffef`, `ss_345bbfc`, `ss_77a1d58`).
    /// Fractures progressively; lateral engine pods cleave off and spin away before the core shatters.
    TwinGunship,
    /// Multi-lobe Sector Command Dreadnought (`radius ~ 9.2`) with severable port/starboard weapon wings.
    SectorDreadnought,
    /// Lightweight resonator drone that surfs Bohmian quantum potential minima (`-grad Q`)
    /// around a Matriarch's carrier wave, scattering naturally when the wave is disrupted.
    BohmianSwarmDrone {
        matriarch_id: u32,
        nodal_slot: u8,
    },
    /// Independent interceptor that hunts Annealing Power Cubes when its hull is fractured.
    AnnealingHunter,
}

/// Field-coupled airborne alien vessel.
#[derive(Debug, Clone, PartialEq)]
pub struct AlienVessel {
    pub role: AlienRole,
    pub hull: CohesiveBody,
    pub fire_cooldown: u16,
    pub carrier_phase: f32,
    pub cubes_healed: u32,
    /// Number of times player weapons have carved/deepened fractures on this hull.
    pub fracture_hits: u16,
    /// Number of times a structural pod/wing has topologically cleaved off this vessel.
    pub cleave_count: u8,
}

impl AlienVessel {
    /// Maximum number of wing/nacelle severing events this vessel class can survive before
    /// further topological cleavage shatters the main hull.
    #[inline]
    pub fn max_survivable_cleaves(&self) -> u8 {
        match self.role {
            AlienRole::SectorDreadnought => 2,
            AlienRole::ChladniMatriarch | AlienRole::TwinGunship => 1,
            _ => 0,
        }
    }

    /// Dynamic field-derived assessment of whether the vessel's lattice has suffered
    /// catastrophic structural failure (through-cleavage, severed load-bearing connectivity,
    /// or cavitated reactor core) and must enter the killed state.
    pub fn is_critically_broken(&self, cleaved: bool, children_len: usize) -> bool {
        let integrity = self.hull.structural_integrity();
        let connected = self.hull.largest_intact_component_ratio();
        let core_coh = self.hull.core_cohesion();
        let active = self.hull.active_cell_count();
        let max_cleaves = self.max_survivable_cleaves();

        // Any topological split beyond allowed wing/pod cleaves, or loss of minimum hull cells
        if (children_len >= 2 && self.cleave_count >= max_cleaves) || active < 10 {
            return true;
        }

        if max_cleaves == 0 {
            // Small vessels (Drones, Hunters): cannot survive through-cleavage or core fracture
            cleaved || connected < 0.62 || integrity < 0.65 || core_coh < 0.50
        } else {
            // Medium & large vessels: survive peripheral wing cleaves, die when the fuselage
            // is split by a wide fracture seam (`connected < 0.48`), loses structural integrity,
            // or suffers reactor core cavitation (`core_coh < 0.42`).
            (cleaved && (self.cleave_count >= max_cleaves || core_coh < 0.55 || integrity < 0.58))
                || connected < 0.48
                || integrity < 0.46
                || core_coh < 0.42
        }
    }

    /// Dynamic check for whether weapons are offline due to reactor core fracture,
    /// low structural integrity, or severed weapon pods.
    pub fn weapons_disabled(&self) -> bool {
        let max_cleaves = self.max_survivable_cleaves();
        (max_cleaves > 0 && self.cleave_count >= max_cleaves)
            || self.hull.core_cohesion() < 0.62
            || self.hull.structural_integrity() < 0.62
            || self.hull.largest_intact_component_ratio() < 0.58
    }
}

/// Destructible ground-layer structure or surface vessel (`Raptor: Call of the Shadows` inspiration).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroundTargetKind {
    /// Faceted metallic fuel sphere (`ss_3ebffef`). Erupts when cleaved, sending an acoustic
    /// shockwave down connected waveguide pipelines to chain-fracture linked structures.
    GeodesicFuelDome,
    /// Large glass-grid dome over an emerald exciton core (`ss_7541140`). Releases Annealing
    /// Power Cubes and high `$` bounty when its fractured roof cleaves open.
    BiodomeReactor,
    /// Twin-barrel ground defense battery (`ss_3ebffef`, `ss_7541140`) that fires flak solitons.
    FlakTurret,
    /// Hydro-canal gunboat (`ss_35965d0`) that traverses water channels and radiates V-wakes.
    CanalPatrolBoat,
}

/// Situated ground structure or surface craft coupled to the scrolling terrain and wavefield.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundTarget {
    pub kind: GroundTargetKind,
    pub hull: CohesiveBody,
    pub fire_cooldown: u16,
    pub fracture_hits: u16,
    /// Shared waveguide pipeline group ID; detonating a dome surges all targets in the group.
    pub pipeline_group: u8,
}

impl GroundTarget {
    /// Dynamic field-derived check for whether a ground target or patrol boat has suffered
    /// catastrophic structural collapse.
    pub fn is_critically_broken(&self, cleaved: bool, children_len: usize) -> bool {
        children_len >= 2
            || (cleaved && self.fracture_hits >= 2)
            || self.hull.largest_intact_component_ratio() < 0.52
            || self.hull.structural_integrity() < 0.50
            || self.hull.core_cohesion() < 0.45
    }

    /// Whether a ground turret or patrol boat's weapons are offline due to heavy structural fracture.
    pub fn weapons_disabled(&self) -> bool {
        self.hull.core_cohesion() < 0.62
            || self.hull.structural_integrity() < 0.62
            || self.hull.largest_intact_component_ratio() < 0.58
    }
}

/// Persistent blast crater stamped into the scrolling terrain after a ground target detonates.
#[derive(Debug, Clone, PartialEq)]
pub struct ScorchedCrater {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub ember_heat: f32,
}

/// Destroyed building rubble embedded on the ground/tarmac around a blast site.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundedWreckage {
    pub body: CohesiveBody,
    pub z: f32,
    pub skid_start_x: f32,
    pub skid_start_y: f32,
    pub ember_heat: f32,
}

/// Annealing Piezo-Condensate ("Power Cube").
///
/// Released when a crystalline asteroid, alien hull, or biodome shatters. Guided by radiation
/// pressure `(-grad p)` and Bohmian quantum potential curvature `(-grad Q)`.
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
    AlienFractured {
        id: u32,
        role: AlienRole,
        fracture_hits: u16,
        severed_shards: usize,
    },
    AlienDestroyed { role: AlienRole, cubes_spawned: usize },
    GroundTargetFractured {
        id: u32,
        kind: GroundTargetKind,
        fracture_hits: u16,
    },
    GroundTargetDestroyed {
        id: u32,
        kind: GroundTargetKind,
        chain_triggered: bool,
        bounty: u32,
    },
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
    pub grounded_wreckage: Vec<GroundedWreckage>,
    pub aliens: Vec<AlienVessel>,
    pub ground_targets: Vec<GroundTarget>,
    pub craters: Vec<ScorchedCrater>,
    pub power_cubes: Vec<PowerCube>,
    pub bolts: Vec<SolitonBolt>,
    pub canyon_scroll: f32,
    pub score: u32,
    pub credits: u32,
    pub wave_number: u32,
}
