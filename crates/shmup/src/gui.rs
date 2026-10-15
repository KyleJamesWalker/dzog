//! Bevy GUI & `dzog_bevy` integration for Crystalline Corridor (`crates/shmup`).

use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use bevy::{
    audio::{AudioSink, AudioSinkPlayback},
    input::gamepad::{Gamepad, GamepadButton},
    prelude::*,
    window::{MonitorSelection, PrimaryWindow, WindowMode},
};
use dzog_bevy::{
    spawn_field_canvas_and_audio, DzogBevyPlugin, DzogFieldAudioSource, FieldImageHandle,
};
use dzog_core::{write_stereo_wav, FieldAcousticFrame, FieldModalSynthesizer};
use crate::field_game::ShmupFieldGame;

pub struct Mp4VideoRecorder {
    pub output_path: String,
    pub temp_video_path: String,
    pub temp_wav_path: String,
    pub child: Child,
    pub stdin: Option<ChildStdin>,
    pub scale: usize,
    pub src_w: usize,
    pub src_h: usize,
    pub upscaled_frame: Vec<u8>,
    pub frames_written: usize,
    pub max_frames: usize,
    pub synth: FieldModalSynthesizer,
    pub audio_left: Vec<f32>,
    pub audio_right: Vec<f32>,
}

impl Mp4VideoRecorder {
    pub fn new(
        output_path: String,
        src_w: usize,
        src_h: usize,
        scale: usize,
        duration_secs: u32,
        shared_audio_frame: Arc<Mutex<FieldAcousticFrame>>,
    ) -> Result<Self, String> {
        let out_w = src_w * scale;
        let out_h = src_h * scale;
        let max_frames = (duration_secs.max(1) as usize) * 60;
        let temp_video_path = format!("{output_path}.video_tmp.mp4");
        let temp_wav_path = format!("{output_path}.audio_tmp.wav");

        let mut child = Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "rawvideo",
                "-pixel_format",
                "rgba",
                "-video_size",
                &format!("{out_w}x{out_h}"),
                "-framerate",
                "60",
                "-i",
                "-",
                "-vf",
                "scale=1280:720:flags=neighbor",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "medium",
                "-crf",
                "25",
                "-maxrate",
                "1600k",
                "-bufsize",
                "3200k",
                &temp_video_path,
            ])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn ffmpeg for MP4 recording: {e}"))?;

        let stdin = child.stdin.take();
        let total_samples = max_frames * (FieldModalSynthesizer::DEFAULT_SAMPLE_RATE as usize / 60);
        Ok(Self {
            output_path,
            temp_video_path,
            temp_wav_path,
            child,
            stdin,
            scale,
            src_w,
            src_h,
            upscaled_frame: vec![0u8; out_w * out_h * 4],
            frames_written: 0,
            max_frames,
            synth: FieldModalSynthesizer::new(shared_audio_frame),
            audio_left: Vec::with_capacity(total_samples),
            audio_right: Vec::with_capacity(total_samples),
        })
    }

    pub fn write_frame(&mut self, rgba_pixels: &[u8]) -> bool {
        let out_w = self.src_w * self.scale;
        let s = self.scale;

        for y in 0..self.src_h {
            for x in 0..self.src_w {
                let src_idx = (y * self.src_w + x) * 4;
                let px = [
                    rgba_pixels[src_idx],
                    rgba_pixels[src_idx + 1],
                    rgba_pixels[src_idx + 2],
                    255,
                ];
                let base_y = y * s;
                let base_x = x * s;
                for dy in 0..s {
                    let row_offset = (base_y + dy) * out_w + base_x;
                    for dx in 0..s {
                        let dst_idx = (row_offset + dx) * 4;
                        self.upscaled_frame[dst_idx..dst_idx + 4].copy_from_slice(&px);
                    }
                }
            }
        }

        if let Some(stdin) = &mut self.stdin {
            let _ = stdin.write_all(&self.upscaled_frame);
        }

        let samples_per_frame = (FieldModalSynthesizer::DEFAULT_SAMPLE_RATE as usize) / 60;
        for _ in 0..samples_per_frame {
            let (l, r) = self.synth.next_stereo_frame();
            self.audio_left.push((l * 0.80).tanh());
            self.audio_right.push((r * 0.80).tanh());
        }

        self.frames_written += 1;
        self.frames_written >= self.max_frames
    }

    pub fn finish(&mut self) {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.flush();
        }
        let _ = self.child.wait();

        let _ = write_stereo_wav(
            &self.temp_wav_path,
            FieldModalSynthesizer::DEFAULT_SAMPLE_RATE,
            &self.audio_left,
            &self.audio_right,
        );

        let mux_status = Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-i",
                &self.temp_video_path,
                "-i",
                &self.temp_wav_path,
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                "-b:a",
                "96k",
                "-movflags",
                "+faststart",
                "-shortest",
                &self.output_path,
            ])
            .status();

        if mux_status.map_or(false, |s| s.success()) {
            let _ = std::fs::remove_file(&self.temp_video_path);
        } else {
            let _ = std::fs::rename(&self.temp_video_path, &self.output_path);
        }
        let _ = std::fs::remove_file(&self.temp_wav_path);

        println!(
            "Saved MP4 recording with stereo field audio to {} ({} frames, {:.1}s)",
            self.output_path,
            self.frames_written,
            self.frames_written as f32 / 60.0
        );
    }
}

#[derive(Resource)]
pub struct ShmupStateResource {
    pub game: ShmupFieldGame,
    pub paused: bool,
    pub fire_cooldown: u8,
    pub fps_ema: f32,
    pub recorder: Option<Mp4VideoRecorder>,
}

#[derive(Component)]
pub struct HudTopText;

#[derive(Component)]
pub struct HudBottomText;

fn parse_cli_options() -> (bool, Option<(String, u32)>) {
    let args: Vec<String> = std::env::args().collect();
    let mut autopilot = args.iter().any(|a| a == "--autopilot");
    let mut record_enabled = false;
    let mut record_path = "shmup_demo.mp4".to_string();
    let mut duration_secs: u32 = 30;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--record" => {
                record_enabled = true;
                if i + 1 < args.len() && !args[i + 1].starts_with("--") {
                    if let Ok(secs) = args[i + 1].parse::<u32>() {
                        duration_secs = secs;
                    } else {
                        record_path = args[i + 1].clone();
                    }
                    i += 1;
                }
            }
            "--duration" | "--seconds" => {
                if i + 1 < args.len() {
                    if let Ok(secs) = args[i + 1].parse::<u32>() {
                        duration_secs = secs;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if record_enabled && !autopilot && args.iter().any(|a| a == "--autopilot") {
        autopilot = true;
    }

    let rec = if record_enabled {
        Some((record_path, duration_secs))
    } else {
        None
    };
    (autopilot, rec)
}

pub fn launch_gui() {
    let (autopilot, record_cfg) = parse_cli_options();
    let mut game = ShmupFieldGame::new(
        ShmupFieldGame::DEFAULT_WIDTH,
        ShmupFieldGame::DEFAULT_HEIGHT,
    );
    game.autopilot_enabled = autopilot;

    if let Some((path, secs)) = record_cfg {
        let scale = 1usize; // 960x540 piped to ffmpeg and scaled cleanly to 1280x720
        let rw = game.render_width();
        let rh = game.render_height();
        println!(
            "Recording Crystalline Corridor session to '{}' (1280x720 + 48kHz AAC audio) for {}s...",
            path, secs
        );
        let shared_audio_for_rec = game.shared_audio_frame.clone();
        match Mp4VideoRecorder::new(path, rw, rh, scale, secs, shared_audio_for_rec) {
            Ok(mut rec) => {
                loop {
                    game.step();
                    game.render_to_pixel_buffer();
                    if rec.write_frame(&game.pixel_buffer) {
                        rec.finish();
                        break;
                    }
                }
            }
            Err(e) => eprintln!("{e}"),
        }
        return;
    }

    let start_windowed = std::env::args().any(|a| a == "--windowed");
    let window_mode = if start_windowed {
        WindowMode::Windowed
    } else {
        WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
    };

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dzog — Shmup".into(),
                resolution: (1920_u32, 1080_u32).into(),
                mode: window_mode,
                canvas: Some("#dzog".into()),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(DzogBevyPlugin)
        .insert_resource(ShmupStateResource {
            game,
            paused: false,
            fire_cooldown: 0,
            fps_ema: 60.0,
            recorder: None,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(
            Update,
            (handle_input, step_simulation, fit_canvas_to_window, update_ui),
        )
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut audio_assets: ResMut<Assets<DzogFieldAudioSource>>,
    mut game_res: ResMut<ShmupStateResource>,
) {
    game_res.game.render_to_pixel_buffer();
    let w = game_res.game.render_width() as u32;
    let h = game_res.game.render_height() as u32;
    let shared_audio = if std::env::args().any(|a| a == "--mute") || game_res.recorder.is_some() {
        None
    } else {
        Some(game_res.game.shared_audio_frame.clone())
    };

    spawn_field_canvas_and_audio(
        &mut commands,
        &mut images,
        &mut audio_assets,
        w,
        h,
        &game_res.game.pixel_buffer,
        Vec2::new(1920.0, 1000.0),
        -40.0,
        shared_audio,
    );

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(80.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.08, 0.94)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("CRYSTALLINE CORRIDOR"),
                TextFont {
                    font_size: FontSize::Px(18.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.95, 1.0)),
                HudTopText,
            ));
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(13.5),
                    ..default()
                },
                TextColor(Color::srgb(0.75, 0.84, 0.94)),
                HudBottomText,
            ));
        });
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut game_res: ResMut<ShmupStateResource>,
    audio_sinks: Query<&AudioSink>,
) {
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyQ) {
        std::process::exit(0);
    }
    if keys.just_pressed(KeyCode::F11) {
        if let Ok(mut win) = windows.single_mut() {
            win.mode = match win.mode {
                WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
                _ => WindowMode::Windowed,
            };
        }
    }

    let mut pad_pause = false;
    let mut pad_view = false;
    let mut pad_dx = 0.0f32;
    let mut pad_dy = 0.0f32;
    let mut pad_fire = false;
    let mut pad_alt_missile = false;
    let mut pad_shield = false;

    // Automatically aggregate input from any connected Gamepad / Controller
    for gamepad in &gamepads {
        if gamepad.just_pressed(GamepadButton::Start) {
            pad_pause = true;
        }
        if gamepad.just_pressed(GamepadButton::Select) {
            pad_view = true;
        }
        let stick = gamepad.left_stick();
        if stick.length() > 0.15 {
            pad_dx += stick.x;
            pad_dy -= stick.y; // Gamepad +Y is stick up -> screen -Y
        }
        if gamepad.pressed(GamepadButton::DPadLeft) {
            pad_dx -= 1.0;
        }
        if gamepad.pressed(GamepadButton::DPadRight) {
            pad_dx += 1.0;
        }
        if gamepad.pressed(GamepadButton::DPadUp) {
            pad_dy -= 1.0;
        }
        if gamepad.pressed(GamepadButton::DPadDown) {
            pad_dy += 1.0;
        }
        if gamepad.pressed(GamepadButton::South)
            || gamepad.pressed(GamepadButton::RightTrigger2)
            || gamepad.pressed(GamepadButton::RightTrigger)
        {
            pad_fire = true;
        }
        if gamepad.just_pressed(GamepadButton::West)
            || gamepad.just_pressed(GamepadButton::East)
            || gamepad.just_pressed(GamepadButton::LeftTrigger2)
        {
            pad_alt_missile = true;
        }
        if gamepad.just_pressed(GamepadButton::North)
            || gamepad.just_pressed(GamepadButton::LeftTrigger)
        {
            pad_shield = true;
        }
    }

    if keys.just_pressed(KeyCode::KeyP) || pad_pause {
        game_res.paused = !game_res.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        game_res.game.reset();
    }
    if keys.just_pressed(KeyCode::KeyV) || pad_view {
        game_res.game.view_mode = game_res.game.view_mode.next();
    }
    if keys.just_pressed(KeyCode::KeyT) {
        game_res.game.autopilot_enabled = !game_res.game.autopilot_enabled;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        if let Ok(sink) = audio_sinks.single() {
            sink.toggle_playback();
        }
    }
    if keys.just_pressed(KeyCode::KeyB) {
        game_res.game.save_branch_checkpoint();
    }
    if keys.just_pressed(KeyCode::KeyU) {
        game_res.game.restore_branch_checkpoint();
    }

    if game_res.paused {
        return;
    }

    // 4-Way WASD / Arrow Keys / Gamepad Left Stick & D-Pad reaction thruster control
    let mut dx = pad_dx;
    let mut dy = pad_dy;
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        dx -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        dx += 1.0;
    }
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        dy -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        dy += 1.0;
    }
    if dx != 0.0 || dy != 0.0 {
        game_res.game.thrust_planar(dx, dy);
    }

    if keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || mouse_buttons.just_pressed(MouseButton::Right)
        || pad_shield
    {
        game_res.game.trigger_antiphase_attractor();
    }

    if game_res.fire_cooldown > 0 {
        game_res.fire_cooldown -= 1;
    }
    let alt_missile_pressed = keys.just_pressed(KeyCode::KeyE)
        || mouse_buttons.just_pressed(MouseButton::Middle)
        || pad_alt_missile;
    if alt_missile_pressed {
        game_res.game.fire_air_to_ground_missiles();
    }

    let fire_pressed =
        keys.pressed(KeyCode::Space) || mouse_buttons.pressed(MouseButton::Left) || pad_fire;
    if fire_pressed && game_res.fire_cooldown == 0 {
        game_res.game.fire_soliton_cannons(false);
        game_res.fire_cooldown = 9;
    }
}

/// Dynamically upscale the 16:9 (1080p-native `960x540`) field canvas to fit 1080p, 1440p, or 4K fullscreen windows.
fn fit_canvas_to_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sprites: Query<(&mut Sprite, &mut Transform)>,
) {
    let Ok(win) = windows.single() else {
        return;
    };
    let hud_h = 80.0f32;
    let avail_w = win.width().max(640.0);
    let avail_h = (win.height() - hud_h).max(360.0);
    let target_aspect = 16.0 / 9.0;

    let (disp_w, disp_h) = if avail_w / avail_h > target_aspect {
        (avail_h * target_aspect, avail_h)
    } else {
        (avail_w, avail_w / target_aspect)
    };

    for (mut sprite, mut transform) in &mut sprites {
        sprite.custom_size = Some(Vec2::new(disp_w, disp_h));
        transform.translation.y = -hud_h * 0.5;
    }
}

fn step_simulation(
    mut game_res: ResMut<ShmupStateResource>,
    image_handle: Res<FieldImageHandle>,
    mut images: ResMut<Assets<Image>>,
) {
    if !game_res.paused {
        game_res.game.step();
    }
    game_res.game.render_to_pixel_buffer();

    if let Some(mut image) = images.get_mut(&image_handle.0) {
        if let Some(data) = &mut image.data {
            data.copy_from_slice(&game_res.game.pixel_buffer);
        }
    }

    let ShmupStateResource {
        ref game,
        ref mut recorder,
        ..
    } = *game_res;

    if let Some(rec) = recorder {
        if rec.write_frame(&game.pixel_buffer) {
            rec.finish();
            *recorder = None;
            std::process::exit(0);
        }
    }
}

fn update_ui(
    time: Res<Time>,
    mut game_res: ResMut<ShmupStateResource>,
    gamepads: Query<&Gamepad>,
    audio_sinks: Query<&AudioSink>,
    mut top_q: Query<&mut Text, (With<HudTopText>, Without<HudBottomText>)>,
    mut bot_q: Query<&mut Text, (With<HudBottomText>, Without<HudTopText>)>,
) {
    let dt = time.delta_secs();
    if dt > 0.0001 {
        let inst_fps = (1.0 / dt).clamp(1.0, 240.0);
        game_res.fps_ema = game_res.fps_ema * 0.90 + inst_fps * 0.10;
    }
    let fps = game_res.fps_ema;
    let g = &game_res.game;
    let ap_status = if g.autopilot_enabled { "ON" } else { "OFF" };
    let pad_status = if !gamepads.is_empty() { "ON" } else { "AUTO" };
    let (p_ok, c_ok, s_ok) = g.player_hull.trilobe_status();
    let lobes_str = format!(
        "[{}{}{}] ({}/3)",
        if p_ok { "L" } else { "-" },
        if c_ok { "C" } else { "-" },
        if s_ok { "R" } else { "-" },
        g.player_intact_lobes()
    );

    if let Ok(mut top) = top_q.single_mut() {
        let alert = if g.player_intact_lobes() < 3 || g.player_hull.mean_cohesion() < 0.70 {
            " | !! SHIELDS LOW !!"
        } else if g.pipeline_flash_timer > 0 {
            " | >> PIPELINE CHAIN DETONATION <<"
        } else {
            ""
        };
        **top = format!(
            "FPS {:3.0} | ${:08} | WAVE {} | HULL {} ({:3.0}%) | SKY {} | GROUND {} | CLEAVES {}{}",
            fps,
            g.credits,
            g.wave_number,
            lobes_str,
            g.player_hull.mean_cohesion() * 100.0,
            g.aliens.len(),
            g.ground_targets.len(),
            g.topological_cleaves,
            alert,
        );
    }

    if let Ok(mut bot) = bot_q.single_mut() {
        let audio_status = match audio_sinks.single() {
            Ok(sink) if !sink.is_paused() => "ON",
            _ => "MUTE",
        };
        **bot = format!(
            "[WASD/Pad:{}] Thrust [Space/RT] Bullets [E/X] Air-Ground Missiles [Shift/LB] Shield [F11] Fullscreen [T] Auto:{} [V] {} [Q/Esc] Quit [N] Audio:{}",
            pad_status,
            ap_status,
            g.view_mode.label(),
            audio_status,
        );
    }
}
