//! Weapons, Progressive Fracture Mechanics & Impact Resolution (`crates/shmup/src/combat.rs`).
//!
//! - **Primary Weapon (`Space` / Left-Click)**: High-force `KineticBullet` projectiles that impart
//!   strong linear knockback (`dP = J`) and rotational torque (`dL_z = r x J`) while carving
//!   progressive fractures into enemy hulls.
//! - **Alt-Fire (`E` / `Q` / Middle-Click)**: Twin `AirToGroundMissile`s (`ss_345bbfc`) that fly
//!   beneath sky squadrons, home toward ground installations, and trigger seismic waveguide
//!   chain-detonations.
//! - **Dynamic Fracture & Wreckage Physics**: Severed enemy wings/nacelles spin off at high speed
//!   and physically smash into other airborne enemies, transferring momentum and fracturing them.

use std::f32::consts::FRAC_PI_2;
use dzog_core::{
    AntiphaseShieldPulse, DirectionalWavePacket, GaussianImpulse, SolitonCarrierPulse,
};

use crate::canyon::canyon_wall_bounds;
use crate::entities::{
    AlienRole, GroundTargetKind, GroundedWreckage, ProjectileKind, ScorchedCrater, ShmupEvent,
    SolitonBolt,
};
use crate::field_game::ShmupFieldGame;

impl ShmupFieldGame {
    /// Inject a multi-lobe thermal + compressional explosion directly into the `(pressure, luminescence)`
    /// lattice so the GPU reaction-diffusion (`luminescence.wgsl`) and FDTD (`pressure.wgsl`) passes
    /// evolve a fiery detonation that cools into billowing dark smoke.
    pub fn inject_fiery_field_explosion(&mut self, x: f32, y: f32, radius: f32, intensity: f32) {
        if !self.wave_force_enabled || y < 2.0 || y >= self.height as f32 - 2.0 {
            return;
        }
        // Moderate acoustic impulse so distant traveling shockwaves don't trigger secondary fire ribbons
        GaussianImpulse::new(x, y, (radius * 0.65).max(1.2), (intensity * 0.28).min(0.95))
            .inject(&mut self.solver.state);

        let w = self.width as isize;
        let h = self.height as isize;
        if self.solver.state.luminescence.len() != (w * h) as usize {
            return;
        }

        // Multi-puff asymmetric fireball + outer smoke skirt seeded into `state.luminescence`
        let lobes = [
            (0.0f32, 0.0f32, 1.0f32, 0.55f32),
            (-radius * 0.48, -radius * 0.32, 0.78, 0.50),
            (radius * 0.50, -radius * 0.24, 0.74, 0.48),
            (-radius * 0.28, radius * 0.46, 0.70, 0.52),
            (radius * 0.38, radius * 0.38, 0.65, 0.50),
        ];
        let r_i = (radius.ceil() as isize) + 3;
        let cx = x.round() as isize;
        let cy = y.round() as isize;

        for &(ox_l, oy_l, gain, width_scale) in &lobes {
            let lx = x + ox_l;
            let ly = y + oy_l;
            let inv_r2 = 1.0 / (radius * width_scale).max(0.9).powi(2);
            for dy in -r_i..=r_i {
                for dx in -r_i..=r_i {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px < 1 || px >= w - 1 || py < 1 || py >= h - 1 {
                        continue;
                    }
                    let d2 = (px as f32 - lx).powi(2) + (py as f32 - ly).powi(2);
                    let env = (-d2 * inv_r2).exp();
                    if env > 0.04 {
                        let idx = (py as usize) * (w as usize) + (px as usize);
                        self.solver.state.luminescence[idx] =
                            (self.solver.state.luminescence[idx].max(intensity * gain * env))
                                .min(4.0);
                    }
                }
            }
        }
    }

    /// Primary Fire (`Spacebar` / Left-Click): High-force kinetic-soliton projectile bullets.
    ///
    /// - Intact wings fire twin wing-mounted bullet streams alongside the central nose cannon.
    /// - Impacts impart strong linear knockback and off-center rotational torque (`r x J`).
    pub fn fire_soliton_cannons(&mut self, request_overcharge: bool) {
        if !self.in_play {
            return;
        }
        let overcharged = request_overcharge && self.capacitor >= 28.0;
        if overcharged {
            self.capacitor = (self.capacitor - 28.0).max(0.0);
        }

        let (port_ok, _, star_ok) = self.player_hull.trilobe_status();
        let forward_doppler = (-self.player_hull.vy).max(0.0) * 0.45;
        let bolt_vy = -(2.45 + forward_doppler + if overcharged { 0.40 } else { 0.0 }).min(2.95);
        let bolt_vx = self.player_hull.vx * 0.18;

        let mut muzzles: Vec<(f32, f32)> = vec![(self.player_hull.x, self.player_hull.y - 5.2)];
        if port_ok {
            muzzles.push((self.player_hull.x - 4.2, self.player_hull.y - 2.5));
        }
        if star_ok {
            muzzles.push((self.player_hull.x + 4.2, self.player_hull.y - 2.5));
        }

        for (mx, my) in muzzles {
            let bx = mx.clamp(6.0, self.width as f32 - 6.0);
            let by = my.clamp(6.0, self.height as f32 - 6.0);

            if self.wave_force_enabled && overcharged {
                let wave_y = (by - 4.0).max(6.0);
                let mut packet = DirectionalWavePacket::new(bx, wave_y, 1.15, 1.65, -FRAC_PI_2);
                packet.inject_wrapped(&mut self.solver.state, &self.solver.material, false);
            }

            self.bolts.push(SolitonBolt {
                x: bx,
                y: by,
                vx: bolt_vx,
                vy: bolt_vy,
                radius: if overcharged { 2.2 } else { 1.6 },
                ttl: 110,
                from_player: true,
                overcharged,
                kind: ProjectileKind::KineticBullet,
            });
        }

        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_player_laser(overcharged);
        }
    }

    /// Alt-Fire (`E` / `Q`): Launch twin finned Air-to-Ground Missiles (`ss_345bbfc`).
    ///
    /// Missiles fly beneath airborne enemies, home onto ground installations (`GeodesicFuelDome`,
    /// `BiodomeReactor`, `FlakTurret`, `CanalPatrolBoat`), and detonate with seismic overpressure.
    pub fn fire_air_to_ground_missiles(&mut self) {
        if !self.in_play {
            return;
        }
        if self.capacitor >= 10.0 {
            self.capacitor = (self.capacitor - 10.0).max(0.0);
        }

        let px = self.player_hull.x;
        let py = self.player_hull.y;
        for side in [-1.0f32, 1.0f32] {
            let mx = (px + side * 3.5).clamp(8.0, self.width as f32 - 8.0);
            let my = (py - 2.2).clamp(8.0, self.height as f32 - 8.0);

            self.bolts.push(SolitonBolt {
                x: mx,
                y: my,
                vx: self.player_hull.vx * 0.25 + side * 0.06,
                vy: -2.05,
                radius: 2.1,
                ttl: 115,
                from_player: true,
                overcharged: false,
                kind: ProjectileKind::AirToGroundMissile,
            });
        }

        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_player_laser(true);
        }
    }

    /// Emit an Antiphase Shield & Bohmian Attractor Pulse (`Shift` or Right-Click).
    pub fn trigger_antiphase_attractor(&mut self) -> f32 {
        let shield = AntiphaseShieldPulse::new(
            self.player_hull.x,
            self.player_hull.y,
            1.8,
            11.0,
            0.88,
        );
        let mut harvested = shield.cancel_and_harvest(
            &mut self.solver.state,
            &self.solver.material,
            false,
        );

        let r_sq = 12.0 * 12.0;
        let sx = self.player_hull.x;
        let sy = self.player_hull.y;
        let mut parried = 0u32;
        self.bolts.retain(|b| {
            if !b.from_player {
                let dx = b.x - sx;
                let dy = b.y - sy;
                if dx * dx + dy * dy <= r_sq {
                    parried += 1;
                    return false;
                }
            }
            true
        });

        if parried > 0 {
            harvested += parried as f32 * 14.0;
            self.score += parried * 75;
        }

        let attractor = GaussianImpulse::new(sx, sy, 3.2, 1.15);
        attractor.inject(&mut self.solver.state);

        for cube in &mut self.power_cubes {
            let dx = sx - cube.body.x;
            let dy = sy - cube.body.y;
            let dist_sq = dx * dx + dy * dy;
            if dist_sq < 46.0 * 46.0 && dist_sq > 1.0 {
                let dist = dist_sq.sqrt();
                cube.body.vx = (cube.body.vx + (dx / dist) * 0.16).clamp(-0.55, 0.55);
                cube.body.vy = (cube.body.vy + (dy / dist) * 0.16).clamp(-0.55, 0.55);
            }
        }

        self.capacitor = (self.capacitor + harvested * 2.2).min(self.max_capacitor);
        self.shield_visual_timer = 14;
        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_shield_pulse(parried > 0);
        }
        harvested
    }

    /// Inflict one structural lobe hit on the player's Tri-Lobe Crystalline Ship.
    pub fn damage_player_lobe(&mut self, hit_x: f32) -> ShmupEvent {
        let (port_ok, core_ok, star_ok) = self.player_hull.trilobe_status();
        if !core_ok {
            self.in_play = false;
            return ShmupEvent::PlayerLobeCleaved { remaining_lobes: 0 };
        }

        let lobe_to_sever = if port_ok && star_ok {
            if hit_x <= self.player_hull.x { -1 } else { 1 }
        } else if port_ok {
            -1
        } else if star_ok {
            1
        } else {
            0
        };

        let (cx, cy) = self.player_hull.sever_trilobe_section(lobe_to_sever);
        if self.wave_force_enabled {
            let blast = GaussianImpulse::new(cx, cy, 2.5, 2.8);
            blast.inject(&mut self.solver.state);
        }

        let remaining = self.player_hull.intact_lobe_count();
        self.invulnerable_steps = 36;
        if remaining == 0 {
            self.in_play = false;
        }

        if let Ok(mut guard) = self.shared_audio_frame.lock() {
            guard.trigger_cavitation_break(0.0, 1.4);
        }

        ShmupEvent::PlayerLobeCleaved {
            remaining_lobes: remaining,
        }
    }

    /// Advance bolts & missiles, resolve impacts, and process flying wreckage-to-enemy collisions.
    pub(crate) fn step_bolts_and_impacts(&mut self, dt_sub: f32, event: &mut ShmupEvent) {
        let old_bolts = std::mem::take(&mut self.bolts);
        let mut kept_bolts = Vec::with_capacity(old_bolts.len());
        let mut rock_hits: Vec<(usize, f32, f32, f32, f32, bool)> = Vec::new();
        let mut alien_hits: Vec<(usize, f32, f32, f32, f32, bool)> = Vec::new();
        let mut ground_hits: Vec<(usize, f32, f32, f32, f32, bool, ProjectileKind)> = Vec::new();
        let mut player_hit_x: Option<f32> = None;

        for mut bolt in old_bolts {
            if bolt.ttl == 0 {
                continue;
            }

            // Air-to-Ground Missiles home decisively toward the nearest ahead ground target
            if bolt.kind == ProjectileKind::AirToGroundMissile {
                if let Some(gt) = self
                    .ground_targets
                    .iter()
                    .filter(|g| g.hull.y > 8.0 && g.hull.y < bolt.y + 16.0)
                    .min_by(|a, b| {
                        let da = (a.hull.x - bolt.x).powi(2) + (a.hull.y - bolt.y).powi(2);
                        let db = (b.hull.x - bolt.x).powi(2) + (b.hull.y - bolt.y).powi(2);
                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                    })
                {
                    let dx = gt.hull.x - bolt.x;
                    let dy = (gt.hull.y - bolt.y).min(-2.0);
                    let dist = (dx * dx + dy * dy).sqrt().max(1.0);
                    let desired_vx = (dx / dist) * 1.45;
                    bolt.vx = (bolt.vx * 0.86 + desired_vx * 0.14).clamp(-1.25, 1.25);
                }
            }

            bolt.x += bolt.vx * dt_sub;
            bolt.y += bolt.vy * dt_sub;

            if bolt.x < 3.0
                || bolt.x >= self.width as f32 - 3.0
                || bolt.y < 2.0
                || bolt.y >= self.height as f32 - 2.0
            {
                continue;
            }

            let by_idx = (bolt.y.round() as usize).clamp(0, self.height - 1);
            let (lw, rw, _) =
                canyon_wall_bounds(by_idx, self.height, self.width, self.canyon_scroll);
            if bolt.x <= lw as f32 + 1.5 {
                bolt.x = lw as f32 + 2.0;
                bolt.vx = bolt.vx.abs().max(0.32);
                if self.wave_force_enabled {
                    GaussianImpulse::new(bolt.x, bolt.y, 1.6, 1.4).inject(&mut self.solver.state);
                }
            } else if bolt.x >= rw as f32 - 1.5 {
                bolt.x = rw as f32 - 2.0;
                bolt.vx = -bolt.vx.abs().max(0.32);
                if self.wave_force_enabled {
                    GaussianImpulse::new(bolt.x, bolt.y, 1.6, 1.4).inject(&mut self.solver.state);
                }
            }

            let mut consumed = false;

            // KineticBullets hit airborne asteroids/wreckage and sky enemies;
            // AirToGroundMissiles fly beneath sky squadrons to strike GroundTargets directly!
            if bolt.kind != ProjectileKind::AirToGroundMissile {
                for (r_idx, rock) in self.asteroids.iter().enumerate() {
                    if rock.y + rock.radius < 0.0 {
                        continue;
                    }
                    let dx = bolt.x - rock.x;
                    let dy = bolt.y - rock.y;
                    if dx * dx + dy * dy <= (rock.radius + bolt.radius).powi(2) {
                        rock_hits
                            .push((r_idx, bolt.x, bolt.y, bolt.vx, bolt.vy, bolt.overcharged));
                        consumed = true;
                        break;
                    }
                }

                if !consumed && bolt.from_player {
                    for (a_idx, alien) in self.aliens.iter().enumerate() {
                        // Allow enemies to enter the visible combat arena (`y >= 12.0`) before taking bullet fire
                        if alien.hull.y < 12.0 {
                            continue;
                        }
                        let dx = bolt.x - alien.hull.x;
                        let dy = bolt.y - alien.hull.y;
                        if dx * dx + dy * dy <= (alien.hull.radius + bolt.radius).powi(2) {
                            alien_hits.push((
                                a_idx,
                                bolt.x,
                                bolt.y,
                                bolt.vx,
                                bolt.vy,
                                bolt.overcharged,
                            ));
                            consumed = true;
                            break;
                        }
                    }
                }
            }

            if !consumed && bolt.from_player {
                for (g_idx, gt) in self.ground_targets.iter().enumerate() {
                    if gt.hull.y + gt.hull.radius < 0.0 {
                        continue;
                    }
                    let dx = bolt.x - gt.hull.x;
                    let dy = bolt.y - gt.hull.y;
                    if dx * dx + dy * dy <= (gt.hull.radius + bolt.radius).powi(2) {
                        ground_hits.push((
                            g_idx,
                            bolt.x,
                            bolt.y,
                            bolt.vx,
                            bolt.vy,
                            bolt.overcharged,
                            bolt.kind,
                        ));
                        consumed = true;
                        break;
                    }
                }
            }

            if !consumed && !bolt.from_player && self.invulnerable_steps == 0 && self.in_play {
                let dx = bolt.x - self.player_hull.x;
                let dy = bolt.y - self.player_hull.y;
                if dx * dx + dy * dy <= (self.player_hull.radius * 0.85 + bolt.radius).powi(2) {
                    player_hit_x = Some(bolt.x);
                    consumed = true;
                }
            }

            if !consumed {
                if self.wave_force_enabled && bolt.overcharged {
                    SolitonCarrierPulse::new(bolt.x, bolt.y, bolt.vx, bolt.vy, 1.25, 0.55, 0.35)
                        .couple_to_lattice(&mut self.solver.state, &self.solver.material, false);
                }
                kept_bolts.push(bolt);
            }
        }
        self.bolts = kept_bolts;

        // 1. Asteroid impacts & cleaving
        if !rock_hits.is_empty() {
            rock_hits.sort_by(|a, b| b.0.cmp(&a.0));
            rock_hits.dedup_by_key(|h| h.0);

            for (r_idx, hx, hy, bvx, bvy, overcharged) in rock_hits {
                if r_idx >= self.asteroids.len() {
                    continue;
                }
                let speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                let dir_x = bvx / speed;
                let dir_y = bvy / speed;
                let energy = if overcharged { 2.2 } else { 1.25 };

                self.asteroids[r_idx].apply_contact_impulse(
                    hx,
                    hy,
                    dir_x * 1.8 * energy,
                    dir_y * 1.8 * energy,
                    None,
                );
                let cleaved = self.asteroids[r_idx]
                    .carve_cleavage_fissure(hx, hy, dir_x, dir_y, energy, None);
                let parent_id = self.asteroids[r_idx].id;
                let class = self.asteroids[r_idx].material.class;

                if self.wave_force_enabled && hy >= 1.0 && hy < self.height as f32 - 1.0 {
                    GaussianImpulse::new(hx, hy, 2.0, 2.2 * energy).inject(&mut self.solver.state);
                }

                let children = self.asteroids[r_idx]
                    .split_connected_components(&mut self.next_body_id, None);
                if children.len() >= 2 {
                    let rx = self.asteroids[r_idx].x;
                    let ry = self.asteroids[r_idx].y;
                    let rvy = self.asteroids[r_idx].vy.abs().max(self.scroll_speed * 0.8);
                    self.asteroids.swap_remove(r_idx);
                    let num_c = children.len();
                    self.asteroids.extend(children);
                    self.topological_cleaves += 1;
                    self.score += 180 * num_c as u32;
                    self.credits += 150 * num_c as u32;
                    self.spawn_power_cube(rx, ry, dir_x * 0.12, rvy);
                    *event = ShmupEvent::AsteroidCleaved {
                        parent_id,
                        children_spawned: num_c,
                    };
                } else if self.asteroids[r_idx].active_cell_count() < 10
                    || (cleaved && self.asteroids[r_idx].radius < 4.5)
                {
                    let removed = self.asteroids.swap_remove(r_idx);
                    let rvy = removed.vy.abs().max(self.scroll_speed * 0.8);
                    self.score += 250;
                    self.credits += 250;
                    self.spawn_power_cube(removed.x - 1.8, removed.y, -0.14, rvy);
                    self.spawn_power_cube(removed.x + 1.8, removed.y, 0.14, rvy);
                    *event = ShmupEvent::AsteroidShattered {
                        body_id: parent_id,
                        cubes_spawned: 2,
                    };
                } else {
                    self.score += 45;
                    self.credits += 50;
                    *event = ShmupEvent::AsteroidCracked {
                        body_id: parent_id,
                        class,
                    };
                }
            }
        }

        // 2. Kinetic-Force Bullet Impacts on Aliens: Knockback, Spin & Wing/Nacelle Cleaving
        if !alien_hits.is_empty() {
            alien_hits.sort_by(|a, b| b.0.cmp(&a.0));
            alien_hits.dedup_by_key(|h| h.0);

            for (a_idx, hx, hy, bvx, bvy, overcharged) in alien_hits {
                if a_idx >= self.aliens.len() {
                    continue;
                }
                let speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                let dir_x = bvx / speed;
                let dir_y = bvy / speed;
                let energy = if overcharged { 2.25 } else { 1.38 };

                // Impart punchy backward & lateral recoil + off-center rotational torque (`r x J`)
                let ay_cur = self.aliens[a_idx].hull.y;
                let offset_x = hx - self.aliens[a_idx].hull.x;
                let lateral_kick = if offset_x.abs() < 0.25 {
                    0.42
                } else {
                    (offset_x * 0.28).clamp(-0.85, 0.85)
                };
                self.aliens[a_idx].hull.apply_contact_impulse(
                    hx,
                    hy,
                    (dir_x + lateral_kick) * 3.4 * energy,
                    -1.45 * energy,
                    None,
                );
                // Allow brief upward recoil jolt when inside the arena (`y > 22.0`) so hits feel forceful
                let min_vy = if ay_cur > 22.0 { -0.22 } else { 0.06 };
                self.aliens[a_idx].hull.vy = self.aliens[a_idx].hull.vy.clamp(min_vy, 0.45);
                self.aliens[a_idx].hull.omega = (self.aliens[a_idx].hull.omega
                    + offset_x * 0.042 * energy)
                    .clamp(-0.18, 0.18);

                self.aliens[a_idx].fracture_hits =
                    self.aliens[a_idx].fracture_hits.saturating_add(1);
                let is_medium_or_large = matches!(
                    self.aliens[a_idx].role,
                    AlienRole::ChladniMatriarch
                        | AlienRole::TwinGunship
                        | AlienRole::SectorDreadnought
                );

                let (cleaved, _existing_cracks) = self.aliens[a_idx]
                    .hull
                    .inflict_progressive_fracture(hx, hy, dir_x, dir_y, energy, None);

                self.inject_fiery_field_explosion(hx, hy, 2.2, 1.55 * energy);

                let mut children = self.aliens[a_idx]
                    .hull
                    .split_connected_components(&mut self.next_body_id, None);

                let max_cleaves = if self.aliens[a_idx].role == AlienRole::SectorDreadnought {
                    2
                } else {
                    1
                };
                if is_medium_or_large
                    && children.len() >= 2
                    && self.aliens[a_idx].cleave_count < max_cleaves
                {
                    children.sort_by_key(|c| std::cmp::Reverse(c.active_cell_count()));
                    if children[0].active_cell_count() >= 14 {
                        let mut surviving_core = children.remove(0);
                        let severed_count = children.len();
                        let core_x = surviving_core.x;

                        for mut shard in children {
                            // Severed wing/nacelle spins off across the sky!
                            let side_sign = if shard.x >= core_x { 1.0 } else { -1.0 };
                            shard.vx = (shard.vx + side_sign * 0.42).clamp(-0.68, 0.68);
                            shard.vy = (shard.vy + 0.10).clamp(0.08, 0.60);
                            shard.omega = side_sign * 0.12;
                            self.asteroids.push(shard);

                            // Equal-and-opposite reaction kick sends the crippled ship careening!
                            surviving_core.vx =
                                (surviving_core.vx - side_sign * 0.28).clamp(-0.60, 0.60);
                            surviving_core.vy = surviving_core.vy.max(0.10);
                            surviving_core.omega = (-side_sign * 0.08).clamp(-0.15, 0.15);
                        }

                        self.aliens[a_idx].hull = surviving_core;
                        self.aliens[a_idx].cleave_count += 1;
                        self.topological_cleaves += 1;
                        self.score += 250 * severed_count as u32;
                        self.credits += 350 * severed_count as u32;
                        self.spawn_power_cube(
                            self.aliens[a_idx].hull.x,
                            self.aliens[a_idx].hull.y.max(2.0),
                            dir_x * 0.12,
                            self.scroll_speed,
                        );
                        *event = ShmupEvent::AlienFractured {
                            id: self.aliens[a_idx].hull.id,
                            role: self.aliens[a_idx].role,
                            fracture_hits: self.aliens[a_idx].fracture_hits,
                            severed_shards: severed_count,
                        };
                        continue;
                    }
                }

                let fatal = if is_medium_or_large {
                    (cleaved && self.aliens[a_idx].fracture_hits >= 2)
                        || self.aliens[a_idx].hull.mean_cohesion() < 0.62
                        || self.aliens[a_idx].hull.active_cell_count() < 12
                        || children.len() >= 2
                } else {
                    cleaved
                        || self.aliens[a_idx].hull.mean_cohesion() < 0.72
                        || self.aliens[a_idx].hull.active_cell_count() < 10
                };

                if fatal {
                    let dead = self.aliens.swap_remove(a_idx);
                    if children.len() >= 2 {
                        self.topological_cleaves += 1;
                    }
                    let bounty = match dead.role {
                        AlienRole::SectorDreadnought => 2000,
                        AlienRole::ChladniMatriarch => 850,
                        AlienRole::TwinGunship => 700,
                        _ => 350,
                    };
                    self.score += bounty;
                    self.credits += bounty;
                    self.inject_fiery_field_explosion(
                        dead.hull.x,
                        dead.hull.y,
                        dead.hull.radius * 0.85,
                        3.6,
                    );
                    let cubes_n = match dead.role {
                        AlienRole::SectorDreadnought => 4,
                        AlienRole::ChladniMatriarch | AlienRole::TwinGunship => 2,
                        _ => 1,
                    };
                    for k in 0..cubes_n {
                        let off = (k as f32 - 0.5) * 2.2;
                        self.spawn_power_cube(
                            dead.hull.x + off,
                            dead.hull.y.max(2.0),
                            off * 0.08,
                            self.scroll_speed * 0.9,
                        );
                    }
                    *event = ShmupEvent::AlienDestroyed {
                        role: dead.role,
                        cubes_spawned: cubes_n,
                    };
                } else {
                    self.score += 80;
                    self.credits += 100;
                    *event = ShmupEvent::AlienFractured {
                        id: self.aliens[a_idx].hull.id,
                        role: self.aliens[a_idx].role,
                        fracture_hits: self.aliens[a_idx].fracture_hits,
                        severed_shards: 0,
                    };
                }
            }
        }

        // 3. Flying Severed Wreckage / Asteroid-to-Alien Collisions!
        for r_idx in 0..self.asteroids.len() {
            let rx = self.asteroids[r_idx].x;
            let ry = self.asteroids[r_idx].y;
            let r_rad = self.asteroids[r_idx].radius;
            let rvx = self.asteroids[r_idx].vx;
            let rvy = self.asteroids[r_idx].vy;
            let r_speed_sq = rvx * rvx + rvy * rvy;
            if r_speed_sq < 0.04 || ry < 0.0 {
                continue;
            }

            for a_idx in 0..self.aliens.len() {
                if self.aliens[a_idx].hull.y < 0.0 {
                    continue;
                }
                let dx = self.aliens[a_idx].hull.x - rx;
                let dy = self.aliens[a_idx].hull.y - ry;
                let min_d = (r_rad + self.aliens[a_idx].hull.radius) * 0.82;
                let d2 = dx * dx + dy * dy;
                if d2 < min_d * min_d && d2 > 0.01 {
                    let d = d2.sqrt();
                    let nx = dx / d;
                    let ny = dy / d;
                    let overlap = min_d - d;
                    self.asteroids[r_idx].x -= nx * overlap * 0.5;
                    self.asteroids[r_idx].y -= ny * overlap * 0.5;
                    self.aliens[a_idx].hull.x += nx * overlap * 0.5;
                    self.aliens[a_idx].hull.y += ny * overlap * 0.5;

                    self.aliens[a_idx]
                        .hull
                        .apply_contact_impulse(rx, ry, nx * 2.2, ny * 2.2, None);
                    self.asteroids[r_idx].vx = (-rvx * 0.65).clamp(-0.55, 0.55);
                    self.aliens[a_idx].fracture_hits =
                        self.aliens[a_idx].fracture_hits.saturating_add(1);
                    self.aliens[a_idx]
                        .hull
                        .inflict_progressive_fracture(rx, ry, nx, ny, 1.55, None);
                }
            }
        }

        // 3b. Enemy-to-Enemy Non-Overlap & Field-Coupled Elastic Bumping!
        let n_aliens = self.aliens.len();
        for i in 0..n_aliens {
            for j in (i + 1)..n_aliens {
                let ax = self.aliens[i].hull.x;
                let ay = self.aliens[i].hull.y;
                let bx = self.aliens[j].hull.x;
                let by = self.aliens[j].hull.y;
                let min_d = (self.aliens[i].hull.radius + self.aliens[j].hull.radius) * 0.98;
                let dx = bx - ax;
                let dy = by - ay;
                let d2 = dx * dx + dy * dy;
                if d2 < min_d * min_d && d2 > 0.001 {
                    let d = d2.sqrt();
                    let nx = dx / d;
                    let ny = dy / d;
                    let overlap = min_d - d;

                    // Separate hulls so enemies never overlap
                    self.aliens[i].hull.x -= nx * overlap * 0.55;
                    self.aliens[i].hull.y -= ny * overlap * 0.55;
                    self.aliens[j].hull.x += nx * overlap * 0.55;
                    self.aliens[j].hull.y += ny * overlap * 0.55;

                    let rvx = self.aliens[j].hull.vx - self.aliens[i].hull.vx;
                    let rvy = self.aliens[j].hull.vy - self.aliens[i].hull.vy;
                    let closing = -(rvx * nx + rvy * ny);
                    let bump = (closing.max(0.12) * 1.65).min(2.4);
                    let cx = 0.5 * (ax + bx);
                    let cy = 0.5 * (ay + by);

                    self.aliens[i]
                        .hull
                        .apply_contact_impulse(cx, cy, -nx * bump, -ny * bump, None);
                    self.aliens[j]
                        .hull
                        .apply_contact_impulse(cx, cy, nx * bump, ny * bump, None);

                    if closing > 0.28 {
                        // High-speed knockback collision cracks both hulls & sparks the field!
                        self.aliens[i]
                            .hull
                            .inflict_progressive_fracture(cx, cy, -nx, -ny, 1.15, None);
                        self.aliens[j]
                            .hull
                            .inflict_progressive_fracture(cx, cy, nx, ny, 1.15, None);
                        self.inject_fiery_field_explosion(cx, cy, 2.0, 1.6);
                    } else if self.wave_force_enabled && cy >= 2.0 && cy < self.height as f32 - 2.0 {
                        GaussianImpulse::new(cx, cy, 1.4, 0.75).inject(&mut self.solver.state);
                    }
                }
            }
        }

        // 4. Ground Target progressive fracturing, Air-to-Ground Missile seismic blasts & waveguide chains
        if !ground_hits.is_empty() {
            ground_hits.sort_by(|a, b| b.0.cmp(&a.0));
            ground_hits.dedup_by_key(|h| h.0);

            let mut triggered_groups: Vec<(u8, f32, f32)> = Vec::new();
            for (g_idx, hx, hy, bvx, bvy, overcharged, p_kind) in ground_hits {
                if g_idx >= self.ground_targets.len() {
                    continue;
                }
                let speed = (bvx * bvx + bvy * bvy).sqrt().max(0.1);
                let dir_x = bvx / speed;
                let dir_y = bvy / speed;
                let is_agm = p_kind == ProjectileKind::AirToGroundMissile;
                let energy = if is_agm {
                    2.65
                } else if overcharged {
                    2.3
                } else {
                    1.45
                };

                self.ground_targets[g_idx].fracture_hits = self.ground_targets[g_idx]
                    .fracture_hits
                    .saturating_add(if is_agm { 2 } else { 1 });
                let (cleaved, _) = self.ground_targets[g_idx]
                    .hull
                    .inflict_progressive_fracture(hx, hy, dir_x, dir_y, energy, None);
                let children = self.ground_targets[g_idx]
                    .hull
                    .split_connected_components(&mut self.next_body_id, None);

                let fatal = is_agm
                    || (cleaved && self.ground_targets[g_idx].fracture_hits >= 2)
                    || self.ground_targets[g_idx].hull.mean_cohesion() < 0.64
                    || children.len() >= 2;

                if fatal {
                    let dead = self.ground_targets.swap_remove(g_idx);
                    // Embed cleaved building structural rubble (`GroundedWreckage`) around the blast crater
                    if !children.is_empty() {
                        for (i, mut shard) in children.into_iter().take(2).enumerate() {
                            let off_x = if i == 0 { -2.8 } else { 2.8 };
                            let off_y = if i == 0 { 1.6 } else { -1.4 };
                            let start_x = dead.hull.x;
                            let start_y = dead.hull.y;
                            shard.x = (start_x + off_x).clamp(14.0, self.width as f32 - 14.0);
                            shard.y = start_y + off_y;
                            shard.theta = (i as f32) * 1.1 + 0.4;
                            if self.grounded_wreckage.len() < 24 {
                                self.grounded_wreckage.push(GroundedWreckage {
                                    body: shard,
                                    z: 0.48,
                                    skid_start_x: start_x,
                                    skid_start_y: start_y,
                                    ember_heat: 1.0,
                                });
                            }
                            self.ground_scars_count += 1;
                        }
                    } else if self.grounded_wreckage.len() < 24 {
                        let mut rubble = dead.hull.clone();
                        rubble.radius *= 0.65;
                        rubble.x = (dead.hull.x + 2.4).clamp(14.0, self.width as f32 - 14.0);
                        rubble.y = dead.hull.y + 1.5;
                        self.grounded_wreckage.push(GroundedWreckage {
                            body: rubble,
                            z: 0.48,
                            skid_start_x: dead.hull.x,
                            skid_start_y: dead.hull.y,
                            ember_heat: 1.0,
                        });
                        self.ground_scars_count += 1;
                    }
                    self.craters.push(ScorchedCrater {
                        x: dead.hull.x,
                        y: dead.hull.y,
                        radius: dead.hull.radius * if is_agm { 1.35 } else { 1.15 },
                        ember_heat: 1.0,
                    });
                    self.inject_fiery_field_explosion(
                        dead.hull.x,
                        dead.hull.y,
                        dead.hull.radius * if is_agm { 1.10 } else { 0.95 },
                        if is_agm { 3.9 } else { 3.4 },
                    );
                    let bounty = match dead.kind {
                        GroundTargetKind::BiodomeReactor => 1200,
                        GroundTargetKind::GeodesicFuelDome => 800,
                        GroundTargetKind::FlakTurret => 550,
                        GroundTargetKind::CanalPatrolBoat => 650,
                    };
                    self.score += bounty;
                    self.credits += bounty;
                    let cubes_n = if dead.kind == GroundTargetKind::BiodomeReactor {
                        2
                    } else {
                        1
                    };
                    for k in 0..cubes_n {
                        let off = (k as f32 - 0.5) * 2.0;
                        self.spawn_power_cube(
                            dead.hull.x + off,
                            dead.hull.y.max(2.0),
                            off * 0.09,
                            self.scroll_speed * 0.95,
                        );
                    }
                    let chain = dead.pipeline_group > 0
                        && matches!(
                            dead.kind,
                            GroundTargetKind::GeodesicFuelDome | GroundTargetKind::BiodomeReactor
                        );
                    if chain {
                        triggered_groups.push((dead.pipeline_group, dead.hull.x, dead.hull.y));
                    }
                    *event = ShmupEvent::GroundTargetDestroyed {
                        id: dead.hull.id,
                        kind: dead.kind,
                        chain_triggered: chain,
                        bounty,
                    };
                } else {
                    {
                        let hull = &mut self.ground_targets[g_idx].hull;
                        for (&occ, c) in hull.occupied.iter().zip(hull.cohesion.iter_mut()) {
                            if occ && *c < 0.22 {
                                *c = 0.22;
                            }
                        }
                    }
                    self.score += 75;
                    self.credits += 90;
                    self.inject_fiery_field_explosion(hx, hy, 2.0, 1.45 * energy);
                    *event = ShmupEvent::GroundTargetFractured {
                        id: self.ground_targets[g_idx].hull.id,
                        kind: self.ground_targets[g_idx].kind,
                        fracture_hits: self.ground_targets[g_idx].fracture_hits,
                    };
                }
            }

            // Propagate sympathetic acoustic surges along connected waveguide pipelines
            for (grp, sx, sy) in triggered_groups {
                self.pipeline_flash_timer = 16;
                for gt in &mut self.ground_targets {
                    if gt.pipeline_group == grp {
                        let dx = gt.hull.x - sx;
                        let dy = gt.hull.y - sy;
                        let ang = dy.atan2(dx);
                        gt.fracture_hits = gt.fracture_hits.saturating_add(1);
                        gt.hull.inflict_progressive_fracture(
                            gt.hull.x,
                            gt.hull.y,
                            ang.cos(),
                            ang.sin(),
                            1.45,
                            None,
                        );
                        for (&occ, c) in gt.hull.occupied.iter().zip(gt.hull.cohesion.iter_mut()) {
                            if occ && *c < 0.22 {
                                *c = 0.22;
                            }
                        }
                        if self.wave_force_enabled
                            && gt.hull.y >= 2.0
                            && gt.hull.y < self.height as f32 - 2.0
                        {
                            let mut pkt =
                                DirectionalWavePacket::new(gt.hull.x, gt.hull.y, 1.8, 0.85, ang);
                            pkt.inject_wrapped(
                                &mut self.solver.state,
                                &self.solver.material,
                                false,
                            );
                        }
                        self.chain_detonations += 1;
                        self.credits += 250;
                    }
                }
            }
        }

        // 5. Asteroid/alien hull collision with player ship
        if player_hit_x.is_none() && self.invulnerable_steps == 0 && self.in_play {
            for rock in &self.asteroids {
                let dx = rock.x - self.player_hull.x;
                let dy = rock.y - self.player_hull.y;
                if dx * dx + dy * dy <= (rock.radius + self.player_hull.radius * 0.72).powi(2) {
                    player_hit_x = Some(rock.x);
                    break;
                }
            }
        }

        if let Some(hx) = player_hit_x {
            *event = self.damage_player_lobe(hx);
        }
    }
}
