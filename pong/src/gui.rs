//! Bevy 0.15 GUI application and retro pixel display for Field-First Solid-State Breakout.

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
    let mut game = BreakoutFieldGame::new(128, 80);
    if !std::env::args().any(|a| a == "--cpu") {
        game.enable_gpu_acceleration();
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Dzoglang — Solid-State & Quantum-Vacuum Breakout".into(),
                resolution: (1024.0_f32, 640.0_f32).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(GameStateResource {
            game,
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
    commands.spawn(Camera2d);

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
            custom_size: Some(Vec2::new(960.0, 580.0)),
            ..default()
        },
        Transform::from_xyz(0.0, -15.0, 0.0),
    ));

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
            BackgroundColor(Color::srgba(0.04, 0.05, 0.09, 0.92)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("SCORE: 00000  |  LIVES: 3  |  BRICKS: 32/32  |  CLEAVES: 0  |  PHASE: 50%"),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgb(0.92, 0.96, 1.0)),
                ScoreText,
            ));

            parent.spawn((
                Text::new("[A/D] Move | [W] Phase Pulse | [T] Sonic Blast & Force: ON | [V] View | [B/U] Branch/Restore"),
                TextFont { font_size: 12.5, ..default() },
                TextColor(Color::srgb(0.55, 0.82, 0.96)),
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

    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        game_res.game.move_paddle(-speed);
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        game_res.game.move_paddle(speed);
    }

    if keyboard.just_pressed(KeyCode::Space) {
        if game_res.game.ball_attached {
            game_res.game.serve();
        } else if !game_res.game.in_play {
            game_res.game.reset_game();
        }
    }

    // Fire coherent upward Phase-Pulse soliton from paddle
    if keyboard.just_pressed(KeyCode::KeyW) || keyboard.just_pressed(KeyCode::ArrowUp) {
        game_res.game.fire_phase_pulse();
    }

    if keyboard.just_pressed(KeyCode::KeyT) {
        game_res.game.toggle_wave_force();
    }

    if keyboard.just_pressed(KeyCode::KeyG) {
        game_res.game.toggle_gpu_acceleration();
    }

    if keyboard.just_pressed(KeyCode::KeyV) {
        game_res.game.cycle_view_mode();
    }

    if keyboard.just_pressed(KeyCode::KeyB) {
        game_res.game.save_branch_checkpoint();
    }
    if keyboard.just_pressed(KeyCode::KeyU) {
        game_res.game.restore_branch_checkpoint();
    }

    if keyboard.just_pressed(KeyCode::KeyP) {
        game_res.paused = !game_res.paused;
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        game_res.game.reset_game();
    }

    if mouse_buttons.just_pressed(MouseButton::Left) || mouse_buttons.pressed(MouseButton::Right) {
        if let Ok(window) = windows.get_single() {
            if let Some(cursor) = window.cursor_position() {
                let win_w = window.width();
                let win_h = window.height();
                let canvas_x = cursor.x - (win_w - 960.0) * 0.5;
                let canvas_y = cursor.y - ((win_h - 580.0) * 0.5 + 15.0);
                if (0.0..=960.0).contains(&canvas_x) && (0.0..=580.0).contains(&canvas_y) {
                    let gx = (canvas_x / 960.0) * game_res.game.width as f32;
                    let gy = (canvas_y / 580.0) * game_res.game.height as f32;
                    if mouse_buttons.just_pressed(MouseButton::Left) {
                        game_res.game.inject_mouse_impulse(gx, gy, 2.8);
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
        game_res.game.step();
    }

    game_res.game.render_to_pixel_buffer();

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
    let lives_str = match game.lives {
        3 => "[3] ♥ ♥ ♥",
        2 => "[2] ♥ ♥ ♡",
        1 => "[1] ♥ ♡ ♡",
        _ => "[0] ♡ ♡ ♡",
    };
    let overcharge_tag = if game.ball_overcharged {
        "  |  ⚡ OVERCHARGED"
    } else {
        ""
    };

    for mut text in &mut score_query {
        text.0 = format!(
            "SCORE: {:05}  |  LIVES: {}  |  BRICKS: {:02}/32  |  CLEAVES: {}  |  PHASE: {:02.0}%{}",
            game.score,
            lives_str,
            game.bricks_remaining,
            game.acoustic_cleaves,
            game.phase_charge,
            overcharge_tag
        );
    }

    for mut text in &mut status_query {
        let blast_str = if game.wave_force_enabled { "ON" } else { "OFF" };
        let gpu_str = if game.is_gpu_accelerated() { "ON" } else { "OFF" };
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
            "[A/D] Move | [W] Phase Pulse | [T] Sonic Blast: {blast_str} | [G] GPU: {gpu_str} | [V] {view_str} | [B/U] Branch({branch_str}){pause_str}{action_hint}"
        );
    }
}
