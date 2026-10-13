//! Bevy 0.15 GUI application and retro pixel display for Field-First Breakout Pong.

use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssetUsages,
        render_resource::{Extent3d, TextureDimension, TextureFormat},
    },
    window::PrimaryWindow,
};
use crate::field_game::BreakoutFieldGame;

#[derive(Resource)]
pub struct GameStateResource {
    pub game: BreakoutFieldGame,
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
                title: "Dzoglang — Field-First Soliton Breakout (Acoustic PDE Engine)".into(),
                resolution: (1024.0_f32, 640.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(GameStateResource {
            game: BreakoutFieldGame::new(128, 80),
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

    // Initial render
    game_res.game.render_to_pixel_buffer();

    let w = game_res.game.width as u32;
    let h = game_res.game.height as u32;

    // Create dynamic retro pixel canvas image
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
    // Nearest-neighbor sampling for crisp retro CRT pixels
    image.sampler = bevy::image::ImageSampler::nearest();

    let image_handle = images.add(image);
    commands.insert_resource(FieldImageHandle(image_handle.clone()));

    // Spawn central retro pixel field canvas quad
    commands.spawn((
        Sprite {
            image: image_handle,
            custom_size: Some(Vec2::new(960.0, 580.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -15.0, 0.0),
    ));

    // UI Overlay: Scoreboard & status header
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(75.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceEvenly,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.1, 0.90)),
        ))
        .with_children(|parent| {
            // Header & Score line
            parent.spawn((
                Text::new("SCORE: 00000  |  LIVES: 3  |  BRICKS: 32/32  |  CLEAVES: 0  |  ENERGY: 0.0 mJ"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
                ScoreText,
            ));

            // Controls & Status line
            parent.spawn((
                Text::new("[A/D] Move | [Space] Launch | [T] Sonic Blast & Force: ON | [V] View | [B/U] Branch/Restore | [L/R Click] Wave/Wall"),
                TextFont { font_size: 12.5, ..default() },
                TextColor(Color::srgb(0.55, 0.8, 0.95)),
                StatusText,
            ));
        });
}

fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut game_res: ResMut<GameStateResource>,
) {
    let speed = 2.4;

    // Paddle movement
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        game_res.game.move_paddle(-speed);
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        game_res.game.move_paddle(speed);
    }

    // Launch ball from paddle
    if keyboard.just_pressed(KeyCode::Space) {
        if game_res.game.ball_attached {
            game_res.game.serve();
        } else if !game_res.game.in_play {
            game_res.game.reset_game();
        }
    }

    // Toggle counterfactual field coupling (sonic blast, wave force, acoustic cleaving)
    if keyboard.just_pressed(KeyCode::KeyT) {
        game_res.game.toggle_wave_force();
    }

    // Cycle perceptual field view mode
    if keyboard.just_pressed(KeyCode::KeyV) {
        game_res.game.cycle_view_mode();
    }

    // Save / restore counterfactual branch checkpoint
    if keyboard.just_pressed(KeyCode::KeyB) {
        game_res.game.save_branch_checkpoint();
    }
    if keyboard.just_pressed(KeyCode::KeyU) {
        game_res.game.restore_branch_checkpoint();
    }

    // Pause toggle
    if keyboard.just_pressed(KeyCode::KeyP) {
        game_res.paused = !game_res.paused;
    }

    // Restart game
    if keyboard.just_pressed(KeyCode::KeyR) {
        game_res.game.reset_game();
    }

    // Interactive mouse field perturbation (Left-click = wave impulse, Right-click = acoustic reflector)
    if mouse_buttons.just_pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right) {
        if let Ok(window) = windows.get_single() {
            if let Some(cursor) = window.cursor_position() {
                let win_w = window.width();
                let win_h = window.height();
                // Map window coordinates to the 960x580 canvas centered at (0, -15)
                let canvas_x = cursor.x - (win_w - 960.0) * 0.5;
                let canvas_y = cursor.y - ((win_h - 580.0) * 0.5 + 15.0);
                if (0.0..=960.0).contains(&canvas_x) && (0.0..=580.0).contains(&canvas_y) {
                    let gx = (canvas_x / 960.0) * game_res.game.width as f32;
                    let gy = (canvas_y / 580.0) * game_res.game.height as f32;
                    if mouse_buttons.just_pressed(MouseButton::Left) {
                        game_res.game.inject_mouse_impulse(gx, gy, 2.6);
                    }
                    if mouse_buttons.pressed(MouseButton::Right) {
                        game_res.game.paint_acoustic_reflector(gx as usize, gy as usize);
                    }
                }
            }
        }
    }
}

fn step_simulation(
    mut game_res: ResMut<GameStateResource>,
    field_handle: Res<FieldImageHandle>,
    mut images: ResMut<Assets<Image>>,
) {
    if !game_res.paused {
        // Advance Dzog field simulation and ball kinematics
        game_res.game.step();
    }

    // Render updated state to pixel buffer
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
    let game = &game_res.game;
    let energy_mj = game.field_energy.total * 1000.0;
    let lives_str = match game.lives {
        3 => "[3] ♥ ♥ ♥",
        2 => "[2] ♥ ♥ ♡",
        1 => "[1] ♥ ♡ ♡",
        _ => "[0] ♡ ♡ ♡",
    };

    for mut text in &mut score_query {
        text.0 = format!(
            "SCORE: {:05}  |  LIVES: {}  |  BRICKS: {:02}/32  |  CLEAVES: {}  |  ENERGY: {:.1} mJ",
            game.score, lives_str, game.bricks_remaining, game.acoustic_cleaves, energy_mj
        );
    }

    for mut text in &mut status_query {
        let blast_str = if game.wave_force_enabled { "ON" } else { "OFF" };
        let view_str = game.view_mode.label();
        let branch_str = if game.saved_branch.is_some() { "SAVED" } else { "NONE" };
        let pause_str = if game_res.paused { " [PAUSED]" } else { "" };

        let action_hint = if !game.in_play {
            if game.bricks_remaining == 0 {
                "  ★ VICTORY! ★ [SPACE/R]"
            } else {
                "  ☠ GAME OVER ☠ [SPACE/R]"
            }
        } else if game.ball_attached {
            "  >>> [SPACE TO LAUNCH] <<<"
        } else {
            ""
        };

        text.0 = format!(
            "[A/D] Move | [T] Sonic Blast & Force: {blast_str} | [V] View: {view_str} | [B/U] Branch({branch_str}) | [P] Pause | [R] Reset{pause_str}{action_hint}"
        );
    }
}
