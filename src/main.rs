mod arsenal;
mod background;
mod boss;
#[cfg(debug_assertions)]
mod devtools;
mod difficulty;
mod enemy;
mod fonts;
mod hud;
mod level;
mod menu;
mod neon;
mod particles;
mod player;
mod quibbo;
mod rng;
mod save;
mod ship;
mod shipyard;
mod shots;
mod stars;
mod sound;
mod superweapon;
mod synth;
mod task;

use bevy::camera::{Hdr, ScalingMode};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};

/// Halbe Breite und Höhe des Spielfelds in Welt-Einheiten. Die Kamera zeigt immer mindestens
/// dieses Feld; auf breiteren Bildschirmen wird es breiter, siehe [`Playfield`].
pub const HALF_WIDTH: f32 = 640.0;
pub const HALF_HEIGHT: f32 = 360.0;

/// Das tatsächlich sichtbare Spielfeld. Auf Breitbild-Monitoren (etwa 21:9) ist es breiter
/// als 16:9; Schiff, Gegner und Boss nutzen dann die ganze Breite.
#[derive(Resource)]
pub struct Playfield {
    pub half_width: f32,
}

impl Default for Playfield {
    fn default() -> Self {
        Self { half_width: HALF_WIDTH }
    }
}

pub fn update_playfield(window: Single<&Window, With<PrimaryWindow>>, mut field: ResMut<Playfield>) {
    let aspect = window.width() / window.height().max(1.0);
    let half_width = (HALF_HEIGHT * aspect).max(HALF_WIDTH);
    if field.half_width != half_width {
        field.half_width = half_width;
    }
}

/// Wo im Spiel man sich befindet. Die Spielwelt läuft nur beim Fliegen.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppState {
    #[default]
    Splash,
    Menu,
    Playing,
    /// Zwischen zwei Flügen: Training, Shop und Abflug.
    Shipyard,
}

/// Alle Systeme der Spielwelt; sie ruhen in Splash und Menü.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Gameplay;

/// Was im Level gerade passiert. Während einer Aufgabe läuft die Welt in Zeitlupe weiter,
/// danach beschleunigt sie in der Rückkehr-Phase wieder auf volles Tempo.
#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GamePhase {
    #[default]
    Flying,
    Task,
    Return,
}

/// Das Spielfenster. Auf dem PC im Vollbild (F11 wechselt), im Browser füllt es die Zeichenfläche
/// der Seite und schluckt die Tasten, damit Leertaste und Pfeile die Seite nicht verschieben.
fn window(test_run: bool, resolution: (u32, u32)) -> Window {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (test_run, resolution);
        Window {
            title: "Nova Numerix".into(),
            canvas: Some("#nova".into()),
            fit_canvas_to_parent: true,
            prevent_default_event_handling: true,
            ..default()
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    Window {
        title: "Nova Numerix".into(),
        resolution: resolution.into(),
        focused: !test_run,
        mode: if test_run { WindowMode::Windowed } else { WindowMode::BorderlessFullscreen(MonitorSelection::Primary) },
        // Testläufe öffnen ihr Fenster außerhalb des Bildschirms, damit es niemanden stört.
        position: if test_run { WindowPosition::At(IVec2::new(-20000, -20000)) } else { WindowPosition::Automatic },
        ..default()
    }
}

fn main() {
    // Automatische Testläufe sollen niemandem den Fokus wegnehmen.
    let test_run = cfg!(debug_assertions) && std::env::var("NOVA_TEST").is_ok();

    // Testläufe können eine andere Fenstergröße verlangen, etwa NOVA_GROESSE=1680x720 für 21:9.
    let resolution = std::env::var("NOVA_GROESSE")
        .ok()
        .filter(|_| test_run)
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?))
        })
        .unwrap_or((1280, 720));

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin { primary_window: Some(window(test_run, resolution)), ..default() })
            // Zu den Bildern gibt es keine .meta-Dateien; im Browser würde jede Nachfrage einen 404 erzeugen.
            .set(AssetPlugin { meta_check: bevy::asset::AssetMetaCheck::Never, ..default() }),
    )
        .insert_resource(ClearColor(Color::srgb(0.005, 0.005, 0.02)))
        // Testläufe starten direkt im Flug, außer NOVA_START verlangt den Splash.
        .insert_state(if test_run && std::env::var("NOVA_START").is_err() { AppState::Playing } else { AppState::Splash })
        .init_state::<GamePhase>()
        .configure_sets(Update, Gameplay.run_if(in_state(AppState::Playing)))
        .init_resource::<rng::Rng>()
        .init_resource::<Shake>()
        .init_resource::<Playfield>()
        .add_systems(PreUpdate, update_playfield)
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, (shake_camera, toggle_fullscreen))
        .add_plugins((
            neon::NeonPlugin,
            particles::ParticlesPlugin,
            background::BackgroundPlugin,
            player::PlayerPlugin,
            shots::ShotsPlugin,
            enemy::EnemyPlugin,
            task::TaskPlugin,
            superweapon::SuperWeaponPlugin,
            level::LevelPlugin,
            boss::BossPlugin,
            hud::HudPlugin,
            menu::MenuPlugin,
        ))
        .add_plugins((shipyard::ShipyardPlugin, ship::ShipPlugin, save::SavePlugin, quibbo::QuibboPlugin, difficulty::DifficultyPlugin, sound::SoundPlugin, fonts::FontsPlugin, stars::StarsPlugin, arsenal::ArsenalPlugin));
    #[cfg(debug_assertions)]
    app.add_plugins(devtools::DevToolsPlugin);
    app.run();
}

/// Stärke des Kamerawackelns in Welt-Einheiten; klingt in Echtzeit ab.
#[derive(Resource, Default)]
pub struct Shake(pub f32);

fn shake_camera(
    time: Res<Time<Real>>,
    mut shake: ResMut<Shake>,
    mut rng: ResMut<rng::Rng>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
) {
    shake.0 = (shake.0 - time.delta_secs() * 30.0).max(0.0);
    camera.translation = (rng.direction() * shake.0).extend(camera.translation.z);
}

fn toggle_fullscreen(keys: Res<ButtonInput<KeyCode>>, mut window: Single<&mut Window, With<PrimaryWindow>>) {
    if !keys.just_pressed(KeyCode::F11) {
        return;
    }
    window.mode = match window.mode {
        WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Current),
        _ => WindowMode::Windowed,
    };
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
