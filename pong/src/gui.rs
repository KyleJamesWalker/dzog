//! Bevy 0.15 GUI application and pixel shader display for Field-First Soliton Pong.

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
};
use crate::field_game::SolitonFieldGame;

#[derive(Resource)]
pub struct GameStateResource {
    pub game: SolitonFieldGame,
    pub ai_enabled: bool,
    pub paused: bool,
}

#[derive(Resource)]
pub struct FieldImageHandle(pub Handle<Image>);

#[derive(Component)]
pub struct ScoreText;

#[derive(Component)]
pub struct StatusText;

pub fn launch_gui() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dzoglang — Field-First Soliton Pong (Atari / C64 Engine)".into(),
                resolution: (1024.0_f32, 640.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(GameStateResource {
            game: SolitonFieldGame::new(128, 64),
            ai_enabled: true,
            paused: false,
        })
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (handle_input, step_simulation, update_ui))
        .run();
}

fn setup_scene(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut game_res: ResMut<GameStateResource>,
) {
    // 2D Camera
    commands.spawn(Camera2d);

    // Initial serve
    game_res.game.serve(2);
    game_res.game.render_to_pixel_buffer();

    let w = game_res.game.width as u32;
    let h = game_res.game.height as u32;

    // Create dynamic pixel canvas image
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
    // Nearest-neighbor sampling for crisp retro pixels
    image.sampler = bevy::image::ImageSampler::nearest();

    let image_handle = images.add(image);
    commands.insert_resource(FieldImageHandle(image_handle.clone()));

    // Spawn central retro pixel field canvas quad
    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(960.0, 480.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -20.0, 0.0),
    ));

    // UI Overlay: Scoreboard & status
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(80.0),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.1, 0.85)),
        ))
        .with_children(|parent| {
            // Header & Score
            parent.spawn((
                Text::new("PLAYER 1: 00   |   DZOGLANG SOLITON FIELD   |   PLAYER 2: 00"),
                TextFont { font_size: 20.0, ..default() },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                ScoreText,
            ));

            // Controls & Status
            parent.spawn((
                Text::new("[W/S] P1  |  [Up/Dn] P2  |  [A] AI: ON  |  [Space] Serve  |  [P] Pause"),
                TextFont { font_size: 14.0, ..default() },
                TextColor(Color::srgb(0.6, 0.8, 0.9)),
                StatusText,
            ));
        });
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut game_res: ResMut<GameStateResource>,
) {
    // Player 1 controls
    if keyboard.pressed(KeyCode::KeyW) {
        game_res.game.move_p1(-2);
    }
    if keyboard.pressed(KeyCode::KeyS) {
        game_res.game.move_p1(2);
    }

    // Player 2 controls (when AI is off)
    if !game_res.ai_enabled {
        if keyboard.pressed(KeyCode::ArrowUp) {
            game_res.game.move_p2(-2);
        }
        if keyboard.pressed(KeyCode::ArrowDown) {
            game_res.game.move_p2(2);
        }
    }

    // AI toggle
    if keyboard.just_pressed(KeyCode::KeyA) {
        game_res.ai_enabled = !game_res.ai_enabled;
    }

    // Serve soliton
    if keyboard.just_pressed(KeyCode::Space) {
        if !game_res.game.in_play {
            game_res.game.serve(2);
        }
    }

    // Pause toggle
    if keyboard.just_pressed(KeyCode::KeyP) {
        game_res.paused = !game_res.paused;
    }

    // Reset field
    if keyboard.just_pressed(KeyCode::KeyR) {
        game_res.game.serve(2);
    }
}

fn step_simulation(
    mut game_res: ResMut<GameStateResource>,
    field_handle: Res<FieldImageHandle>,
    mut images: ResMut<Assets<Image>>,
) {
    if !game_res.paused {
        // AI tracking for player 2 paddle
        if game_res.ai_enabled && game_res.game.in_play {
            let (_, cy) = game_res.game.find_soliton_centroid();
            let p2_center = game_res.game.p2_paddle_y as f32 + game_res.game.paddle_height as f32 / 2.0;
            if cy < p2_center - 1.5 {
                game_res.game.move_p2(-2);
            } else if cy > p2_center + 1.5 {
                game_res.game.move_p2(2);
            }
        }

        // Advance Dzog field simulation
        game_res.game.step();
    }

    // Render field to pixel buffer
    game_res.game.render_to_pixel_buffer();

    // Copy pixels to Bevy GPU texture asset
    if let Some(image) = images.get_mut(&field_handle.0) {
        image.data.copy_from_slice(&game_res.game.pixel_buffer);
    }
}

fn update_ui(
    game_res: Res<GameStateResource>,
    mut score_query: Query<&mut Text, (With<ScoreText>, Without<StatusText>)>,
    mut status_query: Query<&mut Text, (With<StatusText>, Without<ScoreText>)>,
) {
    for mut text in &mut score_query {
        text.0 = format!(
            "PLAYER 1: {:02}   |   DZOGLANG SOLITON FIELD   |   PLAYER 2: {:02}",
            game_res.game.p1_score, game_res.game.p2_score
        );
    }

    for mut text in &mut status_query {
        let ai_str = if game_res.ai_enabled { "ON" } else { "OFF" };
        let pause_str = if game_res.paused { " [PAUSED]" } else { "" };
        text.0 = format!(
            "[W/S] P1 | [Up/Dn] P2 | [A] AI: {ai_str} | [Space] Serve | [P] Pause{pause_str}"
        );
    }
}
