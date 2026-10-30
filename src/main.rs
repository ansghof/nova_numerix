mod background;
mod enemy;
mod hud;
mod neon;
mod particles;
mod player;
mod rng;
mod shots;
mod task;

use bevy::camera::{Hdr, ScalingMode};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

/// Halbe Breite und Höhe des Spielfelds in Welt-Einheiten. Die Kamera zeigt immer mindestens
/// dieses Feld, egal wie groß das Fenster ist.
pub const HALF_WIDTH: f32 = 640.0;
pub const HALF_HEIGHT: f32 = 360.0;

/// Was im Level gerade passiert. Während einer Aufgabe läuft die Welt in Zeitlupe weiter,
/// danach beschleunigt sie in der Rückkehr-Phase wieder auf volles Tempo.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamePhase {
    #[default]
    Flying,
    Task,
    Return,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Nova Numerix".into(),
                resolution: (1280, 720).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.005, 0.005, 0.02)))
        .init_state::<GamePhase>()
        .init_resource::<rng::Rng>()
        .add_systems(Startup, spawn_camera)
        .add_plugins((
            neon::NeonPlugin,
            particles::ParticlesPlugin,
            background::BackgroundPlugin,
            player::PlayerPlugin,
            shots::ShotsPlugin,
            enemy::EnemyPlugin,
            task::TaskPlugin,
            hud::HudPlugin,
        ))
        .run();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        // Leuchtfarben über 1.0 brauchen HDR, damit das Bloom sie zum Glühen bringt.
        Hdr,
        Tonemapping::TonyMcMapface,
        Bloom { intensity: 0.3, ..Bloom::NATURAL },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: HALF_WIDTH * 2.0,
                min_height: HALF_HEIGHT * 2.0,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));
}
