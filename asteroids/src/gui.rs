//! Bevy GUI & `dzog_bevy` integration for Field-First Crystalline Asteroids & Alien Ecology.

use std::io::Write;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use bevy::{
    audio::{AudioSink, AudioSinkPlayback},
    prelude::*,
};
use dzog_bevy::{
    spawn_field_canvas_and_audio, DzogBevyPlugin, DzogFieldAudioSource, FieldImageHandle,
};
use dzog_core::{write_stereo_wav, FieldAcousticFrame, FieldModalSynthesizer};
use crate::field_game::AsteroidsFieldGame;

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
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-preset",
                "medium",
                "-crf",
                "26",
                "-maxrate",
                "600k",
                "-bufsize",
                "1200k",
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

        // Synthesize 800 stereo samples (48000 Hz / 60 FPS) from the live field audio frame
        let samples_per_frame = (FieldModalSynthesizer::DEFAULT_SAMPLE_RATE as usize) / 60;
        for _ in 0..samples_per_frame {
            let (l, r) = self.synth.next_stereo_frame();
            self.audio_left.push(l);
            self.audio_right.push(r);
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
pub struct AsteroidsStateResource {
    pub game: AsteroidsFieldGame,
    pub paused: bool,
    pub fire_cooldown: u8,
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
    let mut record_path = "asteroids_demo.mp4".to_string();
    let mut duration_secs: u32 = 60;

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
    let mut game = AsteroidsFieldGame::new(240, 150);
    game.autopilot_enabled = autopilot;

    let shared_audio_for_rec = game.shared_audio_frame.clone();
    let recorder = record_cfg.and_then(|(path, secs)| {
        let scale = 5usize; // 240x150 * 5 = 1200x750 (<= 1280px wide)
        println!(
            "Recording Asteroids session to '{}' ({}x{} + 48kHz AAC audio) for {}s ({} frames @ 60 FPS)...",
            path,
            game.width * scale,
            game.height * scale,
            secs,
            secs * 60
        );
        Mp4VideoRecorder::new(path, game.width, game.height, scale, secs, shared_audio_for_rec)
            .map_err(|e| eprintln!("{e}"))
            .ok()
    });

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dzog — Field-First Crystalline Asteroids & Alien Ecology".into(),
                resolution: (1600_u32, 1000_u32).into(),
                canvas: Some("#dzog".into()),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(DzogBevyPlugin)
        .insert_resource(AsteroidsStateResource {
            game,
            paused: false,
            fire_cooldown: 0,
            recorder,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (handle_input, step_simulation, update_ui))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut audio_assets: ResMut<Assets<DzogFieldAudioSource>>,
    mut game_res: ResMut<AsteroidsStateResource>,
) {
    game_res.game.render_to_pixel_buffer();
    let w = game_res.game.width as u32;
    let h = game_res.game.height as u32;
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
        Vec2::new(1520.0, 900.0),
        -28.0,
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
                Text::new("ASTEROIDS"),
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
    mut game_res: ResMut<AsteroidsStateResource>,
    audio_sinks: Query<&AudioSink>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        game_res.paused = !game_res.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        game_res.game.reset_game();
    }
    if keys.just_pressed(KeyCode::KeyV) {
        game_res.game.cycle_view_mode();
    }
    if keys.just_pressed(KeyCode::KeyM) {
        game_res.game.toggle_topology_mode();
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

    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        game_res.game.rotate_ship(-1.0);
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        game_res.game.rotate_ship(1.0);
    }

    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        game_res.game.thrust_forward(1.0);
    }

    if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
        game_res.game.apply_field_brake();
    }

    if keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || mouse_buttons.just_pressed(MouseButton::Right)
    {
        game_res.game.trigger_antiphase_shield();
    }

    if game_res.fire_cooldown > 0 {
        game_res.fire_cooldown -= 1;
    }
    let fire_pressed = keys.just_pressed(KeyCode::Space)
        || mouse_buttons.just_pressed(MouseButton::Left)
        || keys.just_pressed(KeyCode::KeyE);
    if fire_pressed && game_res.fire_cooldown == 0 {
        let overcharge = keys.pressed(KeyCode::KeyE);
        game_res.game.fire_soliton_bolt(overcharge);
        game_res.fire_cooldown = 6;
    }
}

fn step_simulation(
    mut game_res: ResMut<AsteroidsStateResource>,
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

    let AsteroidsStateResource {
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
    game_res: Res<AsteroidsStateResource>,
    audio_sinks: Query<&AudioSink>,
    mut top_q: Query<&mut Text, (With<HudTopText>, Without<HudBottomText>)>,
    mut bot_q: Query<&mut Text, (With<HudBottomText>, Without<HudTopText>)>,
) {
    let g = &game_res.game;
    let ap_status = if g.autopilot_enabled { "ON" } else { "OFF" };

    if let Ok(mut top) = top_q.single_mut() {
        **top = format!(
            "WAVE {} | SCORE {:05} | LIVES {} | CAPACITOR {:3.0}% | ROCKS {} | ALIENS {} | CLEAVES {} | PARRIES {}",
            g.wave_number,
            g.score,
            g.lives,
            g.capacitor,
            g.asteroids.len(),
            g.aliens.len(),
            g.topological_cleaves,
            g.shield_parries,
        );
    }

    if let Ok(mut bot) = bot_q.single_mut() {
        let audio_status = match audio_sinks.single() {
            Ok(sink) if !sink.is_paused() => "ON",
            _ => "MUTE",
        };
        **bot = format!(
            "[W/A/D] Thrust/Turn [S] Bow-Brake [Space/E] Soliton/Lance [Shift] Shield [T] AutoPilot:{} [M] {} [V] {} [B/U] Branch [N] Audio:{} ({:.2}/{:.2} f_D {:.2})",
            ap_status,
            g.topology.label(),
            g.view_mode.label(),
            audio_status,
            g.sonifier.rms_left,
            g.sonifier.rms_right,
            g.sonifier.doppler_freq,
        );
    }
}
