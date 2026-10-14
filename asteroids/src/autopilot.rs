//! Basic player-like autopilot for the Asteroids field-physics tech demo (`[T]` or `--autopilot`).
//!
//! Operates on `AsteroidsFieldGame`:
//! - Parries incoming alien soliton bolts using the Antiphase Interference Shield.
//! - Evades or brakes when an asteroid enters close collision range.
//! - Holds steady aim on an alien craft or nearby asteroid using PD angular damping (zero spinning).
//! - Fires short aimed bursts (including Overcharged Phase-Lance shots when capacitor is charged)
//!   and cruises with measured thruster pulses.

use std::f32::consts::{PI, TAU};
use crate::field_game::AsteroidsFieldGame;

impl AsteroidsFieldGame {
    /// Advance one macro-frame of player-like autopilot control.
    pub fn step_autopilot(&mut self) {
        if !self.in_play {
            self.lives = 3;
            self.in_play = true;
            self.invulnerable_steps = 45;
        }

        self.autopilot_tick = self.autopilot_tick.wrapping_add(1);
        let t = self.autopilot_tick;

        // 1. Parry incoming enemy bolts within shield radius
        let bolt_threat = self.bolts.iter().any(|b| {
            if b.from_player {
                return false;
            }
            let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, b.x, b.y);
            dx * dx + dy * dy <= (self.ship_radius + 7.5).powi(2)
        });
        if bolt_threat && self.shield_visual_timer == 0 {
            self.trigger_antiphase_shield();
        }

        // 2. Check for immediate collision hazard to evade or brake
        let mut closest_hazard: Option<(f32, f32, f32)> = None;
        for rock in &self.asteroids {
            let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, rock.x, rock.y);
            let dist = (dx * dx + dy * dy).sqrt();
            let safe_r = rock.radius + self.ship_radius + 11.0;
            if dist < safe_r && closest_hazard.map_or(true, |(_, _, d)| dist < d) {
                closest_hazard = Some((dx, dy, dist));
            }
        }

        let speed = (self.ship_vx * self.ship_vx + self.ship_vy * self.ship_vy).sqrt();

        // 3. Determine desired heading: evade close hazard or aim at a held target
        let (desired_theta, has_aim_target) = if let Some((hdx, hdy, hdist)) = closest_hazard {
            let closing = (self.ship_vx * hdx + self.ship_vy * hdy) / hdist.max(0.5);
            if closing > 0.18 && t % 18 == 0 {
                self.apply_field_brake();
            }
            let away_ang = (-hdy).atan2(-hdx) + 0.55;
            (away_ang, false)
        } else if let Some(alien) = self.aliens.first() {
            let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, alien.hull.x, alien.hull.y);
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let lead_t = (dist / 1.05).clamp(1.0, 28.0);
            let ldx = dx + (alien.hull.vx - self.ship_vx * 0.35) * lead_t;
            let ldy = dy + (alien.hull.vy - self.ship_vy * 0.35) * lead_t;
            (ldy.atan2(ldx), true)
        } else if !self.asteroids.is_empty() {
            // Hold focus on one among the 2 nearest asteroids for ~85 frames so aim stays steady
            let mut ranked: Vec<(usize, f32)> = self
                .asteroids
                .iter()
                .enumerate()
                .map(|(idx, r)| {
                    let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, r.x, r.y);
                    (idx, dx * dx + dy * dy)
                })
                .collect();
            ranked.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            let pick_slot = ((t / 85) as usize) % ranked.len().min(2).max(1);
            let target = &self.asteroids[ranked[pick_slot].0];
            let (dx, dy) = self.delta_vec(self.ship_x, self.ship_y, target.x, target.y);
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let lead_t = (dist / 1.02).clamp(1.0, 30.0);
            let ldx = dx + (target.vx - self.ship_vx * 0.35) * lead_t;
            let ldy = dy + (target.vy - self.ship_vy * 0.35) * lead_t;
            let slight_imperfection = (t as f32 * 0.035).sin() * 0.08;
            (ldy.atan2(ldx) + slight_imperfection, true)
        } else {
            (self.ship_theta, false)
        };

        // 4. Smoothly turn toward desired heading and stop rotating once aligned (no spinning)
        let ang_err = (desired_theta - self.ship_theta + PI).rem_euclid(TAU) - PI;
        let pd_turn = ang_err * 2.2 - self.ship_omega * 3.0;
        if ang_err.abs() > 0.06 {
            self.rotate_ship(pd_turn.clamp(-0.75, 0.75));
        }

        // 5. Measured thruster pulses to cruise around the arena or clear a close rock
        if closest_hazard.is_some() {
            if ang_err.abs() < 0.75 && (t % 12) < 7 {
                self.thrust_forward(0.85);
            }
        } else if speed < 0.30 && ang_err.abs() < 0.50 && (t % 32) < 12 {
            self.thrust_forward(0.78);
        }

        // 6. Fire aimed bursts when lined up on target
        let burst_active = (t % 36) < 22;
        if has_aim_target && burst_active && ang_err.abs() < 0.36 && t % 7 == 0 {
            let overcharge = self.capacitor >= 38.0 && (t % 14 == 0);
            self.fire_soliton_bolt(overcharge);
        }
    }
}
