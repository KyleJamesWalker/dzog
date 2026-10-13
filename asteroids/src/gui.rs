//! Bevy 0.15 GUI & native `bevy_audio` (`Decodable` / `AudioPlayer` / `AudioSink`) integration
//! for Dzoglang Field-First Crystalline Asteroids & Alien Ecology.

use std::sync::{Arc, Mutex};
use bevy::{
    audio::{AddAudioSource, AudioPlayer, AudioSink, AudioSinkPlayback, Decodable, PlaybackSettings, Source},
    prelude::*,
    reflect::TypePath,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use dzog_core::{FieldAcousticFrame, FieldModalSynthesizer};
use crate::field_game::AsteroidsFieldGame;

/// Bevy procedural audio decoder wrapping `dzog_core::FieldModalSynthesizer`.
pub struct DzogFieldDecoder(pub FieldModalSynthesizer);

impl Iterator for DzogFieldDecoder {
    type Item = f32;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Source for DzogFieldDecoder {
    #[inline]
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    #[inline]
    fn channels(&self) -> u16 {
        2
    }

    #[inline]
    fn sample_rate(&self) -> u32 {
        FieldModalSynthesizer::DEFAULT_SAMPLE_RATE
    }

    #[inline]
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

/// Bevy `Asset` + `Decodable` audio source streaming live PDE lattice sonification via `bevy_audio`.
#[derive(Asset, TypePath, Clone)]
pub struct DzogFieldAudioSource {
    pub shared_frame: Arc<Mutex<FieldAcousticFrame>>,
}

impl Decodable for DzogFieldAudioSource {
    type DecoderItem = f32;
    type Decoder = DzogFieldDecoder;

    fn decoder(&self) -> Self::Decoder {
        DzogFieldDecoder(FieldModalSynthesizer::new(self.shared_frame.clone()))
    }
}

#[derive(Resource)]
pub struct AsteroidsStateResource {
    pub game: AsteroidsFieldGame,
    pub paused: bool,
    pub fire_cooldown: u8,
}

#[derive(Resource)]
pub struct FieldImageHandle(pub Handle<Image>);

#[derive(Component)]
pub struct HudTopText;

#[derive(Component)]
pub struct HudBottomText;

pub fn launch_gui() {
    let mut game = AsteroidsFieldGame::new(144, 96);
    if !std::env::args().any(|a| a == "--cpu") {
        game.enable_gpu_acceleration();
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dzoglang — Field-First Crystalline Asteroids & Alien Ecology".into(),
                resolution: (1152.0_f32, 768.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_audio_source::<DzogFieldAudioSource>()
        .insert_resource(AsteroidsStateResource {
            game,
            paused: false,
            fire_cooldown: 0,
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
    commands.spawn(Camera2d);

    // Spawn Bevy native AudioPlayer streaming our PDE FieldModalSynthesizer
    if !std::env::args().any(|a| a == "--mute") {
        let source_handle = audio_assets.add(DzogFieldAudioSource {
            shared_frame: game_res.game.shared_audio_frame.clone(),
        });
        commands.spawn((AudioPlayer(source_handle), PlaybackSettings::LOOP));
    }

    game_res.game.render_to_pixel_buffer();
    let w = game_res.game.width as u32;
    let h = game_res.game.height as u32;

    let mut image = Image::new_fill(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &game_res.game.pixel_buffer,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = bevy::image::ImageSampler::nearest();

    let image_handle = images.add(image);
    commands.insert_resource(FieldImageHandle(image_handle.clone()));

    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(1080.0, 680.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -20.0, 0.0),
    ));

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
                    font_size: 18.0,
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.95, 1.0)),
                HudTopText,
            ));
            parent.spawn((
                Text::new(""),
                TextFont {
                    font_size: 13.5,
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
    if keys.just_pressed(KeyCode::KeyG) {
        game_res.game.toggle_gpu_acceleration();
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        game_res.game.zero_readback_gpu = !game_res.game.zero_readback_gpu;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        if let Ok(sink) = audio_sinks.get_single() {
            sink.toggle();
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

    // Steering (A/D or Left/Right)
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        game_res.game.rotate_ship(-1.0);
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        game_res.game.rotate_ship(1.0);
    }

    // Main Reaction Thruster (W / Up)
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        game_res.game.thrust_forward(1.0);
    }

    // Field-Brake / Phase Anchor Bow-Shock (S / Down)
    if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
        game_res.game.apply_field_brake();
    }

    // Antiphase Interference Shield Parry (ShiftLeft / ShiftRight / Right-Click)
    if keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || mouse_buttons.just_pressed(MouseButton::Right)
    {
        game_res.game.trigger_antiphase_shield();
    }

    // Primary Soliton Bolt / Phase-Lance (Space / Left-Click / KeyE for Overcharge)
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

    if let Some(image) = images.get_mut(&image_handle.0) {
        image.data.copy_from_slice(&game_res.game.pixel_buffer);
    }
}

fn update_ui(
    game_res: Res<AsteroidsStateResource>,
    audio_sinks: Query<&AudioSink>,
    mut top_q: Query<&mut Text, (With<HudTopText>, Without<HudBottomText>)>,
    mut bot_q: Query<&mut Text, (With<HudBottomText>, Without<HudTopText>)>,
) {
    let g = &game_res.game;
    let backend = if g.is_gpu_accelerated() {
        if g.zero_readback_gpu { "WGPU-ZeroReadback" } else { "WGPU" }
    } else {
        "CPU"
    };

    if let Ok(mut top) = top_q.get_single_mut() {
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

    if let Ok(mut bot) = bot_q.get_single_mut() {
        let audio_status = match audio_sinks.get_single() {
            Ok(sink) if !sink.is_paused() => "ON",
            _ => "MUTE",
        };
        **bot = format!(
            "[W/A/D] Thrust/Turn [S] Bow-Brake [Space/E] Soliton/Lance [Shift] Shield [M] {} [V] {} [B/U] Branch [G/Z] {} [N] Audio:{} ({:.2}/{:.2} f_D {:.2})",
            g.topology.label(),
            g.view_mode.label(),
            backend,
            audio_status,
            g.sonifier.rms_left,
            g.sonifier.rms_right,
            g.sonifier.doppler_freq,
        );
    }
}
