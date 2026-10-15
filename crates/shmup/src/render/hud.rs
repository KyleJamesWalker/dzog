//! Diegetic *Raptor: Call of the Shadows* HUD Gauges, Bounty Counter & Ordnance Badge (`crates/shmup/src/render/hud.rs`).

use crate::field_game::ShmupFieldGame;

impl ShmupFieldGame {
    /// Draw *Raptor: Call of the Shadows* style vertical instrument gauges along the left and right edges.
    pub(crate) fn draw_raptor_side_gauges(
        pixel_buffer: &mut [u8],
        rw: usize,
        rh: usize,
        scale: usize,
        hull_ratio: f32,
        cap_ratio: f32,
        wave_ratio: f32,
    ) {
        let top_y = 6 * scale;
        let bot_y = rh.saturating_sub(6 * scale);
        let span = bot_y.saturating_sub(top_y).max(1);
        let bar_w = 3 * scale;

        for y in top_y..bot_y {
            let is_tick = (y / scale) % 2 == 0;
            let fill_frac = (bot_y - y) as f32 / (span as f32);

            for x in scale..=(scale + bar_w) {
                let p_idx = (y * rw + x) * 4;
                let col = if is_tick && fill_frac <= wave_ratio {
                    [235, 165, 45, 255]
                } else {
                    [24, 28, 36, 255]
                };
                pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&col);
            }

            let rx_start = rw.saturating_sub(scale + bar_w + 1);
            let rx_end = rw.saturating_sub(scale + 1);
            let split_x = rx_start + (bar_w * 2 / 3);
            for x in rx_start..=rx_end {
                let p_idx = (y * rw + x) * 4;
                let col = if !is_tick {
                    [22, 26, 34, 255]
                } else if x >= split_x && fill_frac <= cap_ratio {
                    [80, 224, 255, 255]
                } else if x < split_x && fill_frac <= hull_ratio {
                    if hull_ratio < 0.45 {
                        [255, 72, 52, 255]
                    } else {
                        [245, 138, 38, 255]
                    }
                } else {
                    [34, 38, 48, 255]
                };
                pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&col);
            }
        }
    }

    /// Render the iconic *Raptor: Call of the Shadows* top-center `$00053450` orange-gold pixel counter
    /// and top-right Twin Air-to-Ground Missile ordnance badge (`ss_345bbfc`).
    pub(crate) fn draw_raptor_bounty_ticker(
        pixel_buffer: &mut [u8],
        rw: usize,
        rh: usize,
        scale: usize,
        credits: u32,
    ) {
        const GLYPHS: [[u8; 7]; 11] = [
            [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
            [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
            [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
            [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
            [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
            [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
            [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b10001, 0b01110],
            [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
            [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
            [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b01110],
            [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110, 0b00100],
        ];

        let text = format!("${:08}", credits.min(99_999_999));
        let char_w = 6 * scale;
        let total_w = text.len() * char_w;
        let start_x = (rw.saturating_sub(total_w)) / 2;
        let start_y = 3 * scale;

        for (ci, ch) in text.chars().enumerate() {
            let g_idx = if ch == '$' {
                10
            } else {
                (ch as usize).saturating_sub('0' as usize).min(9)
            };
            let glyph = &GLYPHS[g_idx];
            let cx = start_x + ci * char_w;

            for (ry, &row_bits) in glyph.iter().enumerate() {
                for rx in 0..5 {
                    if (row_bits >> (4 - rx)) & 1 == 1 {
                        for oy in -2isize..=(scale as isize + 1) {
                            for ox in -2isize..=(scale as isize + 1) {
                                let px = (cx + rx * scale) as isize + ox;
                                let py = (start_y + ry * scale) as isize + oy;
                                if px >= 0 && px < rw as isize && py >= 0 && py < rh as isize {
                                    let s_idx = (py as usize * rw + px as usize) * 4;
                                    pixel_buffer[s_idx] = 14;
                                    pixel_buffer[s_idx + 1] = 8;
                                    pixel_buffer[s_idx + 2] = 6;
                                }
                            }
                        }
                    }
                }
            }
        }

        for (ci, ch) in text.chars().enumerate() {
            let g_idx = if ch == '$' {
                10
            } else {
                (ch as usize).saturating_sub('0' as usize).min(9)
            };
            let glyph = &GLYPHS[g_idx];
            let cx = start_x + ci * char_w;

            for (ry, &row_bits) in glyph.iter().enumerate() {
                for rx in 0..5 {
                    if (row_bits >> (4 - rx)) & 1 == 1 {
                        let grad = (6usize.saturating_sub(ry) as u8) * 16;
                        for sy in 0..scale {
                            for sx in 0..scale {
                                let px = cx + rx * scale + sx;
                                let py = start_y + ry * scale + sy;
                                if px < rw && py < rh {
                                    let p_idx = (py * rw + px) * 4;
                                    pixel_buffer[p_idx] = 252;
                                    pixel_buffer[p_idx + 1] = 132u8.saturating_add(grad);
                                    pixel_buffer[p_idx + 2] = 24;
                                }
                            }
                        }
                    }
                }
            }
        }

        let badge_w = 13 * scale;
        let badge_h = 11 * scale;
        let bx0 = rw.saturating_sub(badge_w + 8 * scale);
        let by0 = 2 * scale;
        for by in by0..(by0 + badge_h) {
            for bx in bx0..(bx0 + badge_w) {
                if bx >= rw || by >= rh {
                    continue;
                }
                let is_border = bx == bx0
                    || bx + 1 == bx0 + badge_w
                    || by == by0
                    || by + 1 == by0 + badge_h;
                let p_idx = (by * rw + bx) * 4;
                if is_border {
                    pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&[72, 80, 96, 255]);
                } else {
                    pixel_buffer[p_idx] = (pixel_buffer[p_idx] as f32 * 0.35) as u8;
                    pixel_buffer[p_idx + 1] = (pixel_buffer[p_idx + 1] as f32 * 0.35) as u8;
                    pixel_buffer[p_idx + 2] = (pixel_buffer[p_idx + 2] as f32 * 0.35) as u8;
                }
            }
        }
        for m_off in [3 * scale, 8 * scale] {
            let mx = bx0 + m_off;
            for my_rel in (2 * scale)..(9 * scale) {
                for mx_rel in 0..(2 * scale) {
                    let px = mx + mx_rel;
                    let py = by0 + my_rel;
                    if px < rw && py < rh {
                        let p_idx = (py * rw + px) * 4;
                        let col = if my_rel < 4 * scale {
                            [235, 52, 42, 255]
                        } else if my_rel >= 8 * scale {
                            [80, 200, 255, 255]
                        } else {
                            [210, 218, 228, 255]
                        };
                        pixel_buffer[p_idx..p_idx + 4].copy_from_slice(&col);
                    }
                }
            }
        }
    }
}
