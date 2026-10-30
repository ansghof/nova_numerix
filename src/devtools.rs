//! Automatische Testläufe (nur im Debug-Build): Die Umgebungsvariable `NOVA_TEST` enthält ein
//! Skript aus Schritten „Sekunde Aktion“, getrennt durch Semikolons, etwa
//! `NOVA_TEST="2 q; 3 1; 3.1 enter; 3.3 bild falsch; 6 ende"`.
//!
//! Aktionen:
//! - eine Taste: `q`, `enter`, `rück`, `leer`, `0`–`9`
//! - `antwort`: die richtige Lösung der offenen Aufgabe eintippen und abgeben
//! - `bild NAME`: Screenshot nach `target/test-NAME.png`
//! - `ende`: Spiel beenden
//!
//! Das Fenster bekommt dabei keinen Fokus, Tastendrücke werden intern eingespielt.
//! Phasenwechsel und Superwaffen-Zündungen landen im Protokoll.

use std::collections::VecDeque;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use crate::GamePhase;
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
        app.insert_resource(TestScript { steps, keys: VecDeque::new(), held: None })
            .add_systems(PreUpdate, play_script.after(InputSystems))
            .add_systems(Update, (log_phase, log_super));
    }
}

#[derive(Resource)]
struct TestScript {
    steps: VecDeque<(f32, String)>,
    /// Tasten, die nacheinander je ein Bild lang gedrückt werden.
    keys: VecDeque<KeyCode>,
    held: Option<KeyCode>,
}

fn key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "q" => KeyCode::KeyQ,
        "enter" => KeyCode::Enter,
        "rück" => KeyCode::Backspace,
        "leer" => KeyCode::Space,
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
    mut exit: MessageWriter<AppExit>,
) {
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
        } else if action == "ende" {
            exit.write(AppExit::Success);
        } else if action == "antwort" {
            let Some(active) = &active else {
                warn!("Test: keine offene Aufgabe");
                continue;
            };
            let answer = active.task.answer.to_string();
            let digits = answer.chars().filter_map(|c| digit(c.to_digit(10)?));
            script.keys.extend(digits.chain([KeyCode::Enter]));
        } else if let Some(code) = key(&action) {
            script.keys.push_back(code);
        } else {
            warn!("Test: unbekannte Aktion {action}");
        }
    }
}

fn log_phase(phase: Res<State<GamePhase>>, active: Option<Res<ActiveTask>>) {
    if phase.is_changed() {
        let task = active.map(|a| format!(" ({} = {})", a.task.prompt, a.task.answer)).unwrap_or_default();
        info!("Phase: {:?}{task}", phase.get());
    }
}

fn log_super(mut messages: MessageReader<FireSuper>) {
    for fire in messages.read() {
        info!("Superwaffe: Stufe {}, Kraft {:.2}", fire.tier, fire.power);
    }
}
