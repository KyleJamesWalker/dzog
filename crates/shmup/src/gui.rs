//! Bevy GUI & `dzog_bevy` integration for Crystalline Corridor (`crates/shmup`).

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
pub struct ShmupStateResource {
    pub game: ShmupFieldGame,
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
    let mut game = ShmupFieldGame::new(240, 150);
    game.autopilot_enabled = autopilot;

    let shared_audio_for_rec = game.shared_audio_frame.clone();
    let recorder = record_cfg.and_then(|(path, secs)| {
        let scale = 5usize;
        println!(
            "Recording Crystalline Corridor session to '{}' ({}x{} + 48kHz AAC audio) for {}s...",
            path,
            game.width * scale,
            game.height * scale,
            secs
        );
        Mp4VideoRecorder::new(path, game.width, game.height, scale, secs, shared_audio_for_rec)
            .map_err(|e| eprintln!("{e}"))
            .ok()
    });

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dzog — Crystalline Corridor (Bohm-Hiley Chladni Swarm & Annealing Shmup)".into(),
                resolution: (1600_u32, 1000_u32).into(),
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
    mut game_res: ResMut<ShmupStateResource>,
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
    mut game_res: ResMut<ShmupStateResource>,
    audio_sinks: Query<&AudioSink>,
) {
    if keys.just_pressed(KeyCode::KeyP) {
        game_res.paused = !game_res.paused;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        game_res.game.reset();
    }
    if keys.just_pressed(KeyCode::KeyV) {
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

    // 4-Way WASD / Arrow Keys reaction thruster control (W/Up = -Y toward top of screen)
    let mut dx = 0.0f32;
    let mut dy = 0.0f32;
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
    {
        game_res.game.trigger_antiphase_attractor();
    }

    if game_res.fire_cooldown > 0 {
        game_res.fire_cooldown -= 1;
    }
    let fire_pressed = keys.pressed(KeyCode::Space)
        || mouse_buttons.pressed(MouseButton::Left)
        || keys.just_pressed(KeyCode::KeyE);
    if fire_pressed && game_res.fire_cooldown == 0 {
        let overcharge = keys.pressed(KeyCode::KeyE);
        game_res.game.fire_soliton_cannons(overcharge);
        game_res.fire_cooldown = 14;
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
    game_res: Res<ShmupStateResource>,
    audio_sinks: Query<&AudioSink>,
    mut top_q: Query<&mut Text, (With<HudTopText>, Without<HudBottomText>)>,
    mut bot_q: Query<&mut Text, (With<HudBottomText>, Without<HudTopText>)>,
) {
    let g = &game_res.game;
    let ap_status = if g.autopilot_enabled { "ON" } else { "OFF" };
    let (p_ok, c_ok, s_ok) = g.player_hull.trilobe_status();
    let lobes_str = format!(
        "[{}{}{}] ({}/3)",
        if p_ok { "L" } else { "-" },
        if c_ok { "C" } else { "-" },
        if s_ok { "R" } else { "-" },
        g.player_intact_lobes()
    );

    if let Ok(mut top) = top_q.single_mut() {
        **top = format!(
            "WAVE {} | SCORE {:05} | HULL LOBES {} | COHESION {:3.0}% | CUBES {} (HEALED P:{}/A:{}) | SWARM {}",
            g.wave_number,
            g.score,
            lobes_str,
            g.player_hull.mean_cohesion() * 100.0,
            g.power_cubes.len(),
            g.cubes_collected_player,
            g.cubes_collected_aliens,
            g.aliens.len(),
        );
    }

    if let Ok(mut bot) = bot_q.single_mut() {
        let audio_status = match audio_sinks.single() {
            Ok(sink) if !sink.is_paused() => "ON",
            _ => "MUTE",
        };
        **bot = format!(
            "[WASD/Arrows] 4-Way Thrust [Space/E] Soliton/Overcharge [Shift] Antiphase Attractor [T] Auto:{} [V] {} [B/U] Branch [R] Reset [N] Audio:{}",
            ap_status,
            g.view_mode.label(),
            audio_status,
        );
    }
}
