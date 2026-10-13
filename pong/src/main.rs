//! Pong Game Benchmark and Interactive Field Visualizer for Dzoglang.

mod field_pong;
mod rigid_pong;

use std::io::{stdout, Write};
use std::time::{Duration, Instant};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    style::ResetColor,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use field_pong::FieldWavePong;
use rigid_pong::RigidPong;

#[derive(PartialEq, Eq)]
enum PongMode {
    FieldWave,
    RigidAcoustic,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Check if running in headless benchmark mode
    if args.iter().any(|a| a == "--headless" || a == "--bench") {
        run_headless_benchmark();
        return Ok(());
    }

    // Interactive terminal game mode
    let mut stdout = stdout();
    enable_raw_mode()?;
    execute!(stdout, Hide, Clear(ClearType::All))?;

    let result = run_game_loop(&mut stdout);

    execute!(stdout, Show, ResetColor, Clear(ClearType::All), MoveTo(0, 0))?;
    disable_raw_mode()?;

    if let Err(e) = result {
        eprintln!("Error running Pong: {e}");
    }

    Ok(())
}

fn run_headless_benchmark() {
    println!("=== Dzoglang Pong Headless Engine Benchmark ===");
    let mut field_pong = FieldWavePong::new(64, 32);
    let mut rigid_pong = RigidPong::new(64, 32);

    println!("[1/2] Benchmarking Field-Wave Pong (Wave-Packet in 64x32 lattice)...");
    let t0 = Instant::now();
    field_pong.serve(2);
    for _ in 0..500 {
        field_pong.step();
    }
    let d1 = t0.elapsed();
    println!("  Completed 500 macro steps (2,000 substeps) in {:.2?}", d1);

    println!("[2/2] Benchmarking Rigid-Acoustic Pong (Coupled collision FDTD)...");
    let t1 = Instant::now();
    rigid_pong.serve(2);
    for _ in 0..500 {
        rigid_pong.step(1.0);
    }
    let d2 = t1.elapsed();
    println!("  Completed 500 steps (2,000 FDTD substeps + 4,000 CCD steps) in {:.2?}", d2);

    println!("Benchmark finished successfully.");
}

fn run_game_loop(stdout: &mut std::io::Stdout) -> Result<(), Box<dyn std::error::Error>> {
    let court_w = 64;
    let court_h = 24;

    let mut mode = PongMode::FieldWave;
    let mut field_game = FieldWavePong::new(court_w, court_h);
    let mut rigid_game = RigidPong::new(court_w, court_h);
    let mut ai_enabled = true;
    let mut paused = false;

    // Initial serve
    field_game.serve(2);
    rigid_game.serve(2);

    let target_frame_time = Duration::from_millis(33); // ~30 FPS
    let mut last_frame = Instant::now();

    loop {
        // 1. Process Input
        while event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('w') | KeyCode::Char('W') => match mode {
                            PongMode::FieldWave => field_game.move_p1(-1),
                            PongMode::RigidAcoustic => rigid_game.move_p1(-1.5),
                        },
                        KeyCode::Char('s') | KeyCode::Char('S') => match mode {
                            PongMode::FieldWave => field_game.move_p1(1),
                            PongMode::RigidAcoustic => rigid_game.move_p1(1.5),
                        },
                        KeyCode::Up => {
                            if !ai_enabled {
                                match mode {
                                    PongMode::FieldWave => field_game.move_p2(-1),
                                    PongMode::RigidAcoustic => rigid_game.move_p2(-1.5),
                                }
                            }
                        }
                        KeyCode::Down => {
                            if !ai_enabled {
                                match mode {
                                    PongMode::FieldWave => field_game.move_p2(1),
                                    PongMode::RigidAcoustic => rigid_game.move_p2(1.5),
                                }
                            }
                        }
                        KeyCode::Char('a') | KeyCode::Char('A') => {
                            ai_enabled = !ai_enabled;
                        }
                        KeyCode::Char(' ') => match mode {
                            PongMode::FieldWave => {
                                if !field_game.in_play {
                                    field_game.serve(2);
                                }
                            }
                            PongMode::RigidAcoustic => {
                                if !rigid_game.in_play {
                                    rigid_game.serve(2);
                                }
                            }
                        },
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            mode = match mode {
                                PongMode::FieldWave => PongMode::RigidAcoustic,
                                PongMode::RigidAcoustic => PongMode::FieldWave,
                            };
                        }
                        KeyCode::Char('p') | KeyCode::Char('P') => {
                            paused = !paused;
                        }
                        _ => {}
                    }
                }
            }
        }

        // 2. Physics & Engine Step
        if !paused {
            match mode {
                PongMode::FieldWave => {
                    // Simple AI tracker for Player 2
                    if ai_enabled && field_game.in_play {
                        let (_, cy) = field_game.wave_packet_centroid();
                        let p2_center = field_game.p2_paddle_y as f32 + field_game.paddle_height as f32 / 2.0;
                        if cy < p2_center - 1.0 {
                            field_game.move_p2(-1);
                        } else if cy > p2_center + 1.0 {
                            field_game.move_p2(1);
                        }
                    }

                    if let Some(_scorer) = field_game.step() {
                        // Point scored
                    }
                }
                PongMode::RigidAcoustic => {
                    if ai_enabled && rigid_game.in_play {
                        let p2_center = rigid_game.p2_paddle_y + rigid_game.paddle_height / 2.0;
                        if rigid_game.ball_y < p2_center - 1.0 {
                            rigid_game.move_p2(-0.8);
                        } else if rigid_game.ball_y > p2_center + 1.0 {
                            rigid_game.move_p2(0.8);
                        }
                    }

                    if let Some(_scorer) = rigid_game.step(1.0) {
                        // Point scored
                    }
                }
            }
        }

        // 3. Render Court & Field State
        render_court(stdout, &mode, &field_game, &rigid_game, court_w, court_h, ai_enabled, paused)?;

        // Frame timing limiter
        let elapsed = last_frame.elapsed();
        if elapsed < target_frame_time {
            std::thread::sleep(target_frame_time - elapsed);
        }
        last_frame = Instant::now();
    }
}

fn render_court(
    stdout: &mut std::io::Stdout,
    mode: &PongMode,
    field: &FieldWavePong,
    rigid: &RigidPong,
    w: usize,
    h: usize,
    ai: bool,
    paused: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    execute!(stdout, MoveTo(0, 0))?;

    // Header & Scoreboard
    let mode_name = match mode {
        PongMode::FieldWave => "FIELD-WAVE (Acoustic Wave Packet)",
        PongMode::RigidAcoustic => "RIGID-ACOUSTIC (Coupled Impact FDTD)",
    };

    let (s1, s2) = match mode {
        PongMode::FieldWave => (field.p1_score, field.p2_score),
        PongMode::RigidAcoustic => (rigid.p1_score, rigid.p2_score),
    };

    let ai_status = if ai { "ON" } else { "OFF" };
    let pause_status = if paused { " [PAUSED]" } else { "" };

    println!(
        " DZOGLANG PONG BENCHMARK | Mode: {} | AI: {}{}\r",
        mode_name, ai_status, pause_status
    );
    println!(
        " Player 1: {:02}               Scoreboard               Player 2: {:02}\r",
        s1, s2
    );

    // Top border
    print!("+");
    for _ in 0..w {
        print!("-");
    }
    println!("+\r");

    // Court Rows
    for y in 0..h {
        print!("|");
        for x in 0..w {
            let ch = match mode {
                PongMode::FieldWave => {
                    // Check paddles
                    let is_p1 = x == 5 && y >= field.p1_paddle_y && y < field.p1_paddle_y + field.paddle_height;
                    let is_p2 = x == w - 7 && y >= field.p2_paddle_y && y < field.p2_paddle_y + field.paddle_height;

                    if is_p1 || is_p2 {
                        '#'
                    } else if x == w / 2 {
                        ':'
                    } else {
                        // Sample pressure field
                        let idx = field.solver.state.config.index(x, y);
                        let p = field.solver.state.pressure[idx].abs();
                        if p > 0.8 {
                            '@'
                        } else if p > 0.4 {
                            '*'
                        } else if p > 0.15 {
                            '~'
                        } else if p > 0.05 {
                            '.'
                        } else {
                            ' '
                        }
                    }
                }
                PongMode::RigidAcoustic => {
                    let bx = rigid.ball_x.round() as usize;
                    let by = rigid.ball_y.round() as usize;
                    let is_ball = x == bx && y == by;

                    let p1_x = rigid.p1_x.round() as usize;
                    let p2_x = rigid.p2_x.round() as usize;
                    let is_p1 = x == p1_x && y >= rigid.p1_paddle_y.round() as usize && y < (rigid.p1_paddle_y + rigid.paddle_height).round() as usize;
                    let is_p2 = x == p2_x && y >= rigid.p2_paddle_y.round() as usize && y < (rigid.p2_paddle_y + rigid.paddle_height).round() as usize;

                    if is_ball {
                        'O'
                    } else if is_p1 || is_p2 {
                        '#'
                    } else if x == w / 2 {
                        ':'
                    } else {
                        // Acoustic resonance visualization
                        let idx = rigid.solver.state.config.index(x, y);
                        let p = rigid.solver.state.pressure[idx].abs();
                        if p > 0.5 {
                            '*'
                        } else if p > 0.1 {
                            '~'
                        } else {
                            ' '
                        }
                    }
                }
            };
            print!("{ch}");
        }
        println!("|\r");
    }

    // Bottom border
    print!("+");
    for _ in 0..w {
        print!("-");
    }
    println!("+\r");

    // Controls Legend
    println!(
        " Controls: [W/S] P1 Up/Down | [Up/Down] P2 Up/Down | [A] AI Toggle | [Space] Serve\r"
    );
    println!(
        "           [M] Toggle Mode   | [P] Pause           | [Q/Esc] Quit\r"
    );

    stdout.flush()?;
    Ok(())
}
