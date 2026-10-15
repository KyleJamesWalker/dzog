//! Autopilot for Crystalline Corridor (`[T]` or `--autopilot`).
//!
//! Prioritizes Annealing Power Cubes when any structural wing lobe is severed (`intact_lobes < 3`),
//! parries incoming alien bolts with the Antiphase Attractor, and lines up horizontal X alignment
//! at the bottom of the corridor (`y ~ height * 0.80`) to fire fast upward (`-Y`) soliton streams.

use crate::canyon::canyon_wall_bounds;
use crate::field_game::ShmupFieldGame;

impl ShmupFieldGame {
    pub fn step_autopilot(&mut self) {
        if !self.in_play {
            self.reset();
            self.autopilot_enabled = true;
            return;
        }

        self.autopilot_tick = self.autopilot_tick.wrapping_add(1);
        let t = self.autopilot_tick;
        let sx = self.player_hull.x;
        let sy = self.player_hull.y;

        // 1. Parry incoming enemy bolts only at very close range (avoids unnecessary shield rings around player)
        let incoming_bolt = self.bolts.iter().any(|b| {
            !b.from_player && (b.x - sx).powi(2) + (b.y - sy).powi(2) <= 36.0
        });
        if incoming_bolt && self.shield_visual_timer == 0 {
            self.trigger_antiphase_attractor();
        }

        // 2. Evade immediate descending asteroid collision hazard (rock is above player: rock.y < sy)
        let mut evade_dir: Option<(f32, f32)> = None;
        for rock in &self.asteroids {
            let dx = rock.x - sx;
            let dy = sy - rock.y; // positive when rock is above player
            if dy > -4.0 && dy < 26.0 && dx.abs() < rock.radius + self.player_hull.radius + 5.0 {
                let push_x = if sx > self.width as f32 * 0.5 && dx > -2.0 {
                    -1.0
                } else {
                    1.0
                };
                let push_y = if sy < self.height as f32 - 22.0 { 0.6 } else { -0.4 };
                evade_dir = Some((push_x, push_y));
                break;
            }
        }

        if let Some((ex, ey)) = evade_dir {
            self.thrust_planar(ex, ey);
        } else {
            // 3. Intercept nearby Power Cubes in the lower half (`y > height * 0.35`)
            let target_cube = self
                .power_cubes
                .iter()
                .filter(|c| c.body.y > self.height as f32 * 0.35)
                .min_by(|a, b| {
                    let da = (a.body.x - sx).powi(2) + (a.body.y - sy).powi(2);
                    let db = (b.body.x - sx).powi(2) + (b.body.y - sy).powi(2);
                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                });

            if let Some(cube) = target_cube {
                let dx = cube.body.x - sx;
                let dy = (cube.body.y + 2.0) - sy;
                self.thrust_planar(dx.clamp(-1.0, 1.0), dy.clamp(-0.8, 0.8));
            } else {
                // 4. Sweep across both sky squadrons (`TwinGunship`, `ChladniMatriarch`) and ground domes
                let focus_ground = (t / 90) % 2 == 1;
                let target_x = if focus_ground {
                    self.ground_targets
                        .iter()
                        .find(|g| g.hull.y > 16.0 && g.hull.y < sy - 10.0)
                        .map(|g| g.hull.x)
                } else {
                    None
                }
                .or_else(|| {
                    self.aliens
                        .iter()
                        .find(|a| a.hull.y > 14.0)
                        .map(|a| a.hull.x)
                })
                .or_else(|| {
                    self.ground_targets
                        .iter()
                        .find(|g| g.hull.y > 10.0)
                        .map(|g| g.hull.x)
                })
                .unwrap_or(self.width as f32 * 0.5);
                let desired_y = self.height as f32 * 0.80 + (t as f32 * 0.04).sin() * 5.0;
                let (lw, rw, _) = canyon_wall_bounds(
                    (sy.round() as usize).clamp(0, self.height - 1),
                    self.height,
                    self.width,
                    self.canyon_scroll,
                );
                let safe_tx = target_x.clamp(lw as f32 + 12.0, rw as f32 - 12.0);
                let dx = safe_tx - sx;
                let dy = desired_y - sy;
                if dx.abs() > 1.5 || dy.abs() > 1.5 {
                    self.thrust_planar((dx * 0.18).clamp(-1.0, 1.0), (dy * 0.15).clamp(-0.7, 0.7));
                }
            }
        }

        // 5. Fire kinetic-force projectile bullets every 14 ticks & Air-to-Ground Missiles every 34 ticks
        if t % 14 == 0 {
            let overcharge = self.capacitor >= 42.0 && (t % 42 == 0);
            self.fire_soliton_cannons(overcharge);
        }
        if t % 34 == 8 && self.ground_targets.iter().any(|g| g.hull.y > 10.0) {
            self.fire_air_to_ground_missiles();
        }
    }
}
