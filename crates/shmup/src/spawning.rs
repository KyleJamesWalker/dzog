//! Wave & Sector Spawning for *Shmup* (`crates/shmup/src/spawning.rs`).
//!
//! Tuned as a brisk "Level 2" (*Bravo-Tango Hydro-Foundry Sector*) experience:
//! - Immediate dual-layer combat on Wave 1 (fortified ground canal base + sky squadrons).
//! - Escalating wave pacing:
//!   - **Wave 1**: Chladni Swarm Matriarch + 3 Bohmian Drones + Twin Gunship + Annealing Hunter +
//!     5-target ground pipeline complex.
//!   - **Wave 2**: Twin-Gunship Pincer formation + Chladni Swarm + Dual Flak Crossfire Battery.
//!   - **Wave 3+**: Sector Command Dreadnought (`radius = 9.2`, dual severable weapon pods) escorted
//!     by strike gunships and fortified biodome/fuel-dome clusters, with progressive scroll acceleration.

use std::f32::consts::FRAC_PI_2;
use dzog_core::{CohesiveBody, MineralClass};

use crate::entities::{AlienRole, AlienVessel, GroundTarget, GroundTargetKind, PowerCube};
use crate::field_game::ShmupFieldGame;

impl ShmupFieldGame {
    /// Reset after core cavitation or manual reset (`[R]`).
    pub fn reset(&mut self) {
        self.solver.state.clear();
        self.player_hull = CohesiveBody::new_trilobe_ship(
            1,
            self.width as f32 * 0.5,
            self.height as f32 * 0.82,
            6.5,
        );
        self.invulnerable_steps = 35;
        self.shield_visual_timer = 0;
        self.thruster_visual_timer = 0;
        self.capacitor = 35.0;
        self.asteroids.clear();
        self.grounded_wreckage.clear();
        self.aliens.clear();
        self.ground_targets.clear();
        self.craters.clear();
        self.power_cubes.clear();
        self.bolts.clear();
        self.score = 0;
        self.credits = 0;
        self.wave_number = 1;
        self.scroll_speed = 0.24;
        self.topological_cleaves = 0;
        self.chain_detonations = 0;
        self.ground_scars_count = 0;
        self.cubes_collected_player = 0;
        self.cubes_collected_aliens = 0;
        self.in_play = true;
        self.spawn_timer = 0;
        self.pipeline_flash_timer = 0;
        self.spawn_wave_formation(1);
        self.sync_all_bodies_to_lattice();
    }

    /// Spawn an escalating "Level 2" wave of sky squadrons and ground base infrastructure
    /// **offscreen above the top horizon (`y < 0`)** (plus initial mid-screen ground installations on Wave 1).
    pub fn spawn_wave_formation(&mut self, wave: u32) {
        let w = self.width as f32;
        // Brisk Level 2 scroll tempo ramping smoothly with wave number
        self.scroll_speed = (0.24 + (wave.saturating_sub(1) as f32) * 0.012).min(0.34);

        // 1. Chladni Swarm Matriarch + Bohmian Swarm Drones (`y < 0`)
        let matriarch_id = self.next_body_id;
        self.next_body_id += 1;
        let mx = (w * 0.5 + ((wave as f32 * 1.3).sin() * w * 0.20)).clamp(40.0, w - 40.0);
        let my = -10.0;
        let mut m_hull = CohesiveBody::new_weaver_ship(matriarch_id, mx, my, 6.2);
        m_hull.vy = self.scroll_speed * 0.94;
        m_hull.theta = FRAC_PI_2;
        self.aliens.push(AlienVessel {
            role: AlienRole::ChladniMatriarch,
            hull: m_hull,
            fire_cooldown: 60usize.saturating_sub((wave as usize) * 3).max(38) as u16,
            carrier_phase: 0.0,
            cubes_healed: 0,
            fracture_hits: 0,
            cleave_count: 0,
        });

        let drone_count = (3 + (wave as usize % 2)).min(4);
        for slot in 0..drone_count {
            let d_id = self.next_body_id;
            self.next_body_id += 1;
            let ang = FRAC_PI_2 + (slot as f32 - (drone_count as f32 - 1.0) * 0.5) * 0.78;
            let r_nodal = 17.5;
            let dx = (mx + ang.cos() * r_nodal).clamp(26.0, w - 26.0);
            let dy = my - 8.0 - (slot as f32 * 3.5) + ang.sin() * r_nodal * 0.5;
            let mut d_hull = CohesiveBody::new_saucer(d_id, dx, dy, 3.1);
            d_hull.vy = self.scroll_speed * 0.94;
            d_hull.theta = FRAC_PI_2;
            self.aliens.push(AlienVessel {
                role: AlienRole::BohmianSwarmDrone {
                    matriarch_id,
                    nodal_slot: slot as u8,
                },
                hull: d_hull,
                fire_cooldown: 50 + (slot as u16) * 20,
                carrier_phase: slot as f32 * 1.1,
                cubes_healed: 0,
                fracture_hits: 0,
                cleave_count: 0,
            });
        }

        // 2. Twin-Nacelle Strike Gunships (1 on Wave 1, 2-ship Pincer on Wave 2+)
        let gunship_lanes: &[f32] = if wave >= 2 {
            &[0.26, 0.74]
        } else if mx > w * 0.5 {
            &[0.28]
        } else {
            &[0.72]
        };
        for (g_i, &lane_frac) in gunship_lanes.iter().enumerate() {
            let gunship_id = self.next_body_id;
            self.next_body_id += 1;
            let gx = (w * lane_frac).clamp(36.0, w - 36.0);
            let gy = -16.0 - (g_i as f32 * 11.0);
            let mut g_hull = CohesiveBody::new_twin_gunship(gunship_id, gx, gy, 6.8);
            g_hull.vy = self.scroll_speed * 0.98;
            g_hull.theta = FRAC_PI_2;
            self.aliens.push(AlienVessel {
                role: AlienRole::TwinGunship,
                hull: g_hull,
                fire_cooldown: 48 + (g_i as u16) * 16,
                carrier_phase: g_i as f32 * 0.9,
                cubes_healed: 0,
                fracture_hits: 0,
                cleave_count: 0,
            });
        }

        // Fast Annealing Hunter flanker on every wave (keeps Level 2 pressure dynamic)
        let hunter_id = self.next_body_id;
        self.next_body_id += 1;
        let hx = if wave % 2 == 0 { w * 0.38 } else { w * 0.62 };
        let mut h_hull = CohesiveBody::new_saucer(hunter_id, hx, -22.0, 3.6);
        h_hull.vy = self.scroll_speed * 1.08;
        h_hull.theta = FRAC_PI_2;
        self.aliens.push(AlienVessel {
            role: AlienRole::AnnealingHunter,
            hull: h_hull,
            fire_cooldown: 62,
            carrier_phase: 0.4,
            cubes_healed: 0,
            fracture_hits: 0,
            cleave_count: 0,
        });

        // 3. Sector Command Dreadnought on every 3rd wave (or Wave 3+)
        if wave % 3 == 0 {
            let boss_id = self.next_body_id;
            self.next_body_id += 1;
            let mut boss_hull = CohesiveBody::new_twin_gunship(boss_id, w * 0.5, -28.0, 9.2);
            boss_hull.vy = self.scroll_speed * 0.78;
            boss_hull.theta = FRAC_PI_2;
            self.aliens.push(AlienVessel {
                role: AlienRole::SectorDreadnought,
                hull: boss_hull,
                fire_cooldown: 44,
                carrier_phase: 0.0,
                cubes_healed: 0,
                fracture_hits: 0,
                cleave_count: 0,
            });
        }

        // 4. Fortified Ground Base Infrastructure positioned along the central canal decks (`x = 58..182`)
        let pipe_group = (wave as u8).wrapping_mul(3).wrapping_add(1);
        let dome_left_x = 64.0 + ((wave % 2) as f32) * 16.0;
        let dome_right_x = w - 64.0 - (((wave + 1) % 2) as f32) * 16.0;
        let y_base = if wave == 1 { 38.0 } else { -8.0 };

        // West Geodesic Fuel Dome
        let d1_id = self.next_body_id;
        self.next_body_id += 1;
        let mut d1_hull = CohesiveBody::new_geodesic_dome(
            d1_id,
            dome_left_x,
            y_base,
            6.5,
            MineralClass::BismuthCrystal,
        );
        d1_hull.vy = self.scroll_speed;
        self.ground_targets.push(GroundTarget {
            kind: GroundTargetKind::GeodesicFuelDome,
            hull: d1_hull,
            fire_cooldown: 0,
            fracture_hits: 0,
            pipeline_group: pipe_group,
        });

        // East Biodome Reactor (`ss_7541140`)
        let bio_id = self.next_body_id;
        self.next_body_id += 1;
        let mut bio_hull = CohesiveBody::new_geodesic_dome(
            bio_id,
            dome_right_x,
            y_base - 14.0,
            7.4,
            MineralClass::PiezoQuartz,
        );
        bio_hull.vy = self.scroll_speed;
        self.ground_targets.push(GroundTarget {
            kind: GroundTargetKind::BiodomeReactor,
            hull: bio_hull,
            fire_cooldown: 0,
            fracture_hits: 0,
            pipeline_group: pipe_group,
        });

        // West & East Flak Turret Emplacements (linked to the pipeline group)
        for (t_idx, tx) in [
            (dome_left_x + 20.0).clamp(48.0, w - 48.0),
            (dome_right_x - 20.0).clamp(48.0, w - 48.0),
        ]
        .into_iter()
        .enumerate()
        {
            let tur_id = self.next_body_id;
            self.next_body_id += 1;
            let ty = if t_idx == 0 {
                y_base - 7.0
            } else {
                y_base - 22.0
            };
            let mut tur_hull =
                CohesiveBody::new_geodesic_dome(tur_id, tx, ty, 4.6, MineralClass::ObsidianAnvil);
            tur_hull.vy = self.scroll_speed;
            self.ground_targets.push(GroundTarget {
                kind: GroundTargetKind::FlakTurret,
                hull: tur_hull,
                fire_cooldown: 52 + (t_idx as u16) * 24,
                fracture_hits: 0,
                pipeline_group: pipe_group,
            });
        }

        // Hydro-Canal Patrol Boat (`ss_35965d0`) cruising down the central water channel
        let boat_id = self.next_body_id;
        self.next_body_id += 1;
        let mut boat_hull = CohesiveBody::new_saucer(boat_id, w * 0.5, y_base + 12.0, 4.8);
        boat_hull.vx = if wave % 2 == 0 { 0.065 } else { -0.065 };
        boat_hull.vy = self.scroll_speed * 1.05;
        boat_hull.theta = FRAC_PI_2;
        self.ground_targets.push(GroundTarget {
            kind: GroundTargetKind::CanalPatrolBoat,
            hull: boat_hull,
            fire_cooldown: 70,
            fracture_hits: 0,
            pipeline_group: 0,
        });

        // 5. Floating Armored Barrage Mines (`ss_35965d0`)
        let num_mines = 2;
        for i in 0..num_mines {
            let r_id = self.next_body_id;
            self.next_body_id += 1;
            let rx = if i == 0 { w * 0.34 } else { w * 0.66 };
            let ry = -9.0 - (i as f32 * 12.0);
            let mut mine = CohesiveBody::new_asteroid(
                r_id,
                rx,
                ry,
                4.4,
                8,
                100 + wave * 31 + i as u32 * 17,
                MineralClass::ObsidianAnvil,
            );
            mine.vx = ((i as f32 * 1.7 + wave as f32).sin()) * 0.05;
            mine.vy = self.scroll_speed + 0.03;
            mine.omega = if i % 2 == 0 { 0.018 } else { -0.018 };
            self.asteroids.push(mine);
        }
    }

    /// Spawn one Annealing Piezo-Condensate ("Power Cube") at `(x, y)` with initial velocity `(vx, vy)`.
    pub fn spawn_power_cube(&mut self, x: f32, y: f32, vx: f32, vy: f32) {
        if self.power_cubes.len() >= 4 {
            return;
        }
        let id = self.next_body_id;
        self.next_body_id += 1;
        let mut body = CohesiveBody::new_power_cube(id, x, y);
        body.vx = vx.clamp(-0.45, 0.45);
        body.vy = vy.clamp(-0.45, 0.45);
        self.power_cubes.push(PowerCube {
            body,
            ttl: 280,
            annealing_energy: 28.0,
        });
    }
}
