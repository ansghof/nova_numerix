//! Automatische Testläufe (nur im Debug-Build): Die Umgebungsvariable `NOVA_TEST` enthält ein
//! Skript aus Schritten „Sekunde Aktion“, getrennt durch Semikolons, etwa
//! `NOVA_TEST="2 q; 3 1; 3.1 enter; 3.3 bild falsch; 6 ende"`.
//!
//! Aktionen:
//! - eine Taste: `q`, `enter`, `rück`, `leer`, `esc`, `hoch`, `runter`, `0`–`9`
//! - `antwort`: die richtige Lösung der offenen Aufgabe eintippen und abgeben
//! - `boss`: direkt ans Levelende springen
//! - `sieg`: den Boss sofort besiegen
//! - `geld N`: N Credits aufs Konto
//! - `tippe TEXT`: Text eintippen, etwa den Schiffsnamen
//! - `feuer an` / `feuer aus`: Leertaste dauerhaft halten
//! - `zielen an` / `zielen aus`: das Schiff auf die richtige Bossplatte oder den offenen Kern ausrichten
//! - `bild NAME`: Screenshot nach `target/test-NAME.png`
//! - `wellenbild NAME`: Screenshot kurz nach der nächsten ausgelöschten Welle
//! - `ende`: Spiel beenden
//!
//! Das Fenster bekommt dabei keinen Fokus, Tastendrücke werden intern eingespielt.
//! Phasenwechsel und Superwaffen-Zündungen landen im Protokoll.

use std::collections::VecDeque;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::GamePhase;
use crate::shipyard::{Credits, Shipyard};
use crate::boss::{Boss, BossCore, Plate};
use crate::enemy::WaveCleared;
use crate::level::{LEVEL_LENGTH, LevelProgress};
use crate::player::Player;
use crate::superweapon::FireSuper;
use crate::task::ActiveTask;

pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, app: &mut App) {
        let Ok(script) = std::env::var("NOVA_TEST") else {
            return;
        };
        let steps = script
            .split(';')
            .filter_map(|step| {
                let step = step.trim();
                let (time, action) = step.split_once(' ')?;
                Some((time.parse().ok()?, action.trim().to_string()))
            })
            .collect();
        app.insert_resource(TestScript { steps, keys: VecDeque::new(), held: None, fire: false, aim: false })
            .add_systems(PreUpdate, (play_script, aim).chain().after(InputSystems))
            .init_resource::<WaveShot>()
            .add_systems(Update, (log_phase, log_super, log_waves));
    }
}

#[derive(Resource)]
struct TestScript {
    steps: VecDeque<(f32, String)>,
    /// Tasten, die nacheinander je ein Bild lang gedrückt werden.
    keys: VecDeque<KeyCode>,
    held: Option<KeyCode>,
    fire: bool,
    aim: bool,
}

fn key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "q" => KeyCode::KeyQ,
        "enter" => KeyCode::Enter,
        "rück" => KeyCode::Backspace,
        "leer" => KeyCode::Space,
        "esc" => KeyCode::Escape,
        "hoch" => KeyCode::ArrowUp,
        "runter" => KeyCode::ArrowDown,
        "w" => KeyCode::KeyW,
        "a" => KeyCode::KeyA,
        "s" => KeyCode::KeyS,
        "d" => KeyCode::KeyD,
        "v" => KeyCode::KeyV,
        _ => digit(name.parse().ok()?)?,
    })
}

fn digit(d: u32) -> Option<KeyCode> {
    [
        KeyCode::Numpad0,
        KeyCode::Numpad1,
        KeyCode::Numpad2,
        KeyCode::Numpad3,
        KeyCode::Numpad4,
        KeyCode::Numpad5,
        KeyCode::Numpad6,
        KeyCode::Numpad7,
        KeyCode::Numpad8,
        KeyCode::Numpad9,
    ]
    .get(d as usize)
    .copied()
}

fn play_script(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut script: ResMut<TestScript>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    active: Option<Res<ActiveTask>>,
    shipyard: Option<Res<Shipyard>>,
    mut boss: Option<Single<&mut Boss>>,
    mut credits: ResMut<Credits>,
    mut progress: ResMut<LevelProgress>,
    mut exit: MessageWriter<AppExit>,
    mut typed: MessageWriter<bevy::input::keyboard::KeyboardInput>,
    window: Single<Entity, With<bevy::window::PrimaryWindow>>,
) {
    if script.fire {
        keys.press(KeyCode::Space);
    }
    if let Some(held) = script.held.take() {
        keys.release(held);
    }
    if let Some(next) = script.keys.pop_front() {
        keys.press(next);
        script.held = Some(next);
    }

    while script.steps.front().is_some_and(|(at, _)| *at <= time.elapsed_secs()) {
        let (_, action) = script.steps.pop_front().unwrap();
        info!("Test: {action}");
        if let Some(name) = action.strip_prefix("bild ") {
            let path = format!("target/test-{name}.png");
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        } else if let Some(name) = action.strip_prefix("wellenbild ") {
            commands.insert_resource(WaveShot { name: Some(name.to_string()), countdown: None });
        } else if let Some(text) = action.strip_prefix("tippe ") {
            for c in text.chars() {
                let s = c.to_string();
                typed.write(bevy::input::keyboard::KeyboardInput {
                    key_code: KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
                    logical_key: bevy::input::keyboard::Key::Character(s.as_str().into()),
                    state: bevy::input::ButtonState::Pressed,
                    text: Some(s.as_str().into()),
                    repeat: false,
                    window: *window,
                });
            }
        } else if action == "sieg" {
            if let Some(boss) = &mut boss {
                boss.health = 0.0;
            }
        } else if let Some(amount) = action.strip_prefix("geld ").and_then(|a| a.parse::<u32>().ok()) {
            credits.bank += amount;
        } else if action == "boss" {
            progress.elapsed = LEVEL_LENGTH;
        } else if let Some(on) = action.strip_prefix("feuer ") {
            script.fire = on == "an";
            if !script.fire {
                keys.release(KeyCode::Space);
            }
        } else if let Some(on) = action.strip_prefix("zielen ") {
            script.aim = on == "an";
        } else if action == "ende" {
            exit.write(AppExit::Success);
        } else if action == "antwort" {
            let open = active.as_ref().map(|a| a.task.answer).or_else(|| shipyard.as_ref().and_then(|s| s.open_answer()));
            let Some(answer) = open else {
                warn!("Test: keine offene Aufgabe");
                continue;
            };
            let answer = answer.to_string();
            let digits = answer.chars().filter_map(|c| digit(c.to_digit(10)?));
            script.keys.extend(digits.chain([KeyCode::Enter]));
        } else if let Some(code) = key(&action) {
            script.keys.push_back(code);
        } else {
            warn!("Test: unbekannte Aktion {action}");
        }
    }
}

/// Hält das Schiff auf der Höhe dessen, was gerade getroffen werden soll.
fn aim(
    script: Res<TestScript>,
    boss: Option<Single<&Boss>>,
    plates: Query<(&Plate, &GlobalTransform)>,
    core: Option<Single<&GlobalTransform, With<BossCore>>>,
    mut player: Single<&mut Transform, With<Player>>,
) {
    let (true, Some(boss)) = (script.aim, boss) else {
        return;
    };
    let target = if boss.asking() {
        plates.iter().find(|(plate, _)| plate.correct).map(|(_, t)| t.translation().y)
    } else if boss.core_open() {
        core.map(|c| c.translation().y)
    } else {
        None
    };
    if let Some(y) = target {
        player.translation.y = y;
    }
}

fn log_phase(phase: Res<State<GamePhase>>, active: Option<Res<ActiveTask>>) {
    if phase.is_changed() {
        let task = active.map(|a| format!(" ({})", a.task.solution())).unwrap_or_default();
        info!("Phase: {:?}{task}", phase.get());
    }
}

fn log_super(mut messages: MessageReader<FireSuper>) {
    for fire in messages.read() {
        info!("Superwaffe: Stufe {}, Kraft {:.2}", fire.tier, fire.power);
    }
}

/// Screenshot, der auf die nächste ausgelöschte Welle wartet.
#[derive(Resource, Default)]
struct WaveShot {
    name: Option<String>,
    countdown: Option<f32>,
}

fn log_waves(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut messages: MessageReader<WaveCleared>,
    mut shot: ResMut<WaveShot>,
) {
    for wave in messages.read() {
        info!("Welle ausgelöscht: {} Gegner", wave.size);
        if shot.name.is_some() && shot.countdown.is_none() {
            shot.countdown = Some(0.25);
        }
    }
    let Some(countdown) = &mut shot.countdown else {
        return;
    };
    *countdown -= time.delta_secs();
    if *countdown <= 0.0 {
        let name = shot.name.take().unwrap_or_default();
        shot.countdown = None;
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(format!("target/test-{name}.png")));
    }
}
