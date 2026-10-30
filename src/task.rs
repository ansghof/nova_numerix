//! Rechenaufgaben im Flug: Eine abgeschossene Frachtdrohne öffnet ein Fenster mit einer
//! Aufgabe, die Welt läuft derweil in Zeitlupe weiter. Danach beschleunigt sie in der
//! Rückkehr-Phase wieder, damit die rechte Hand Zeit hat, zurück an die Steuerung zu finden.
//!
//! Falsch ist immer schlechter als langsam: Wer richtig rechnet, bekommt die Belohnung und
//! Punkte fürs Tempo; wem die Zeit ausgeht, bekommt die Belohnung trotzdem; wer falsch rechnet,
//! bekommt nichts und einen leichten, nie tödlichen Treffer.

use bevy::prelude::*;

use crate::GamePhase;
use crate::enemy::FreightDestroyed;
use crate::hud::Score;
use crate::player::{Loadout, MAX_HEALTH, MAX_LEVEL, Player};
use crate::rng::Rng;
use crate::superweapon::FireSuper;

/// Weltgeschwindigkeit während einer Aufgabe.
const SLOW_MOTION: f32 = 0.1;
/// So lange (in echten Sekunden) bremst die Welt beim Öffnen auf Zeitlupe ab.
const SLOW_DOWN_TIME: f32 = 0.2;
/// So lange beschleunigt sie danach wieder auf volles Tempo.
const RETURN_TIME: f32 = 0.7;
/// Anschließend blinkt das Schiff noch so lange unverwundbar.
const AFTER_RETURN_INVULNERABLE: f32 = 1.0;
const WRONG_DAMAGE: f32 = 10.0;
const REPAIR_AMOUNT: f32 = 35.0;
const BASE_POINTS: f32 = 100.0;
const SPEED_POINTS: f32 = 400.0;
/// Ab diesem Anteil an Restzeit gilt eine Antwort als blitzschnell.
const FAST_SHARE: f32 = 0.5;
const MAX_DIGITS: usize = 4;
/// Die Superwaffe hat etwas mehr Zeit, weil ein Fehler eine zweite Aufgabe nach sich zieht.
const SUPER_EXTRA_TIME: f32 = 3.0;

const TEXT: Color = Color::srgb(0.92, 0.96, 1.0);
const DIM: Color = Color::srgb(0.55, 0.62, 0.75);
const GOOD: Color = Color::srgb(0.45, 1.0, 0.6);
const OKAY: Color = Color::srgb(1.0, 0.85, 0.35);
const BAD: Color = Color::srgb(1.0, 0.4, 0.4);
const FRAME: Color = Color::srgb(0.3, 0.85, 1.0);

pub struct TaskPlugin;

impl Plugin for TaskPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Topic(Box::new(SmallTimesTable::new())))
            .init_resource::<PendingTasks>()
            .add_systems(OnEnter(GamePhase::Task), spawn_popup)
            .add_systems(OnExit(GamePhase::Return), close_popup)
            .add_systems(
                Update,
                (
                    count_freight,
                    start_task.run_if(in_state(GamePhase::Flying)),
                    type_answer.run_if(in_state(GamePhase::Task)),
                    finish_return.run_if(in_state(GamePhase::Return)),
                    world_speed,
                )
                    .chain(),
            );
    }
}

pub struct Task {
    /// Die Aufgabe ohne Ergebnis, etwa „7 × 8“.
    pub prompt: String,
    pub answer: u32,
    /// Kennung des Rechenfakts, damit schwache Fakten öfter wiederkommen.
    pub fact: usize,
    /// Zeitlimit in echten Sekunden.
    pub time_limit: f32,
}

pub enum Outcome {
    Correct { remaining_share: f32 },
    TimedOut,
    Wrong,
}

/// Ein Thema, aus dem Aufgaben gezogen werden. Ein neues Level-Thema ist eine neue
/// Implementierung.
pub trait TaskGenerator: Send + Sync + 'static {
    fn next(&mut self, rng: &mut Rng) -> Task;
    /// Wie die Aufgabe ausging, damit das Thema schwache Fakten öfter stellt.
    fn record(&mut self, task: &Task, outcome: &Outcome);
}

/// Kleines Einmaleins von 2 × 2 bis 10 × 10. Jedes Fakt hat ein Gewicht: Fehler erhöhen es,
/// schnelle richtige Antworten senken es.
pub struct SmallTimesTable {
    weights: Vec<f32>,
    last: Option<usize>,
}

impl SmallTimesTable {
    const LOW: u32 = 2;
    const SPAN: usize = 9;

    pub fn new() -> Self {
        Self { weights: vec![1.0; Self::SPAN * Self::SPAN], last: None }
    }

    /// Dieselbe Aufgabe wie zuletzt, auch vertauscht: Nach 10 × 3 soll nicht 3 × 10 kommen.
    fn repeats_last(&self, fact: usize) -> bool {
        self.last.is_some_and(|last| {
            let swapped = (last % Self::SPAN) * Self::SPAN + last / Self::SPAN;
            fact == last || fact == swapped
        })
    }
}

impl TaskGenerator for SmallTimesTable {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let total: f32 = self.weights.iter().enumerate().filter(|(i, _)| !self.repeats_last(*i)).map(|(_, w)| w).sum();
        let mut pick = rng.f32() * total;
        let mut fact = 0;
        for (i, w) in self.weights.iter().enumerate() {
            if self.repeats_last(i) {
                continue;
            }
            fact = i;
            if pick < *w {
                break;
            }
            pick -= w;
        }
        self.last = Some(fact);
        let a = Self::LOW + (fact / Self::SPAN) as u32;
        let b = Self::LOW + (fact % Self::SPAN) as u32;
        Task { prompt: format!("{a} × {b}"), answer: a * b, fact, time_limit: 6.0 }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        let w = &mut self.weights[task.fact];
        *w = match outcome {
            Outcome::Correct { remaining_share } if *remaining_share >= FAST_SHARE => *w * 0.6,
            Outcome::Correct { .. } => *w * 0.85,
            Outcome::TimedOut => *w * 1.6,
            Outcome::Wrong => *w * 2.5,
        }
        .clamp(0.2, 8.0);
    }
}

#[derive(Resource)]
pub struct Topic(pub Box<dyn TaskGenerator>);

/// Aufgaben, die starten, sobald gerade keine läuft.
#[derive(Resource, Default)]
pub struct PendingTasks {
    /// Abgeschossene Frachtdrohnen.
    freight: u32,
    /// Der Spieler will die Superwaffe zünden.
    pub super_weapon: bool,
}

/// Wofür gerechnet wird.
enum Purpose {
    Freight(Reward),
    /// Bei einem Fehler gibt es eine zweite Aufgabe; die Zeit läuft dabei weiter.
    Super { second_chance_used: bool },
}

#[derive(Clone, Copy)]
enum Reward {
    Laser,
    Missiles,
    Repair,
}

impl Reward {
    fn name(self) -> &'static str {
        match self {
            Self::Laser => "Laser +1",
            Self::Missiles => "Raketen +1",
            Self::Repair => "Reparatur",
        }
    }

    /// Eine Belohnung, die gerade etwas bringt: keine Waffe über der Höchststufe, Reparatur
    /// bevorzugt bei angeschlagenem Schiff.
    fn choose(rng: &mut Rng, loadout: &Loadout, health: f32) -> Self {
        if health < MAX_HEALTH * 0.5 && rng.chance(0.6) {
            return Self::Repair;
        }
        let mut options = Vec::new();
        if loadout.laser < MAX_LEVEL {
            options.push(Self::Laser);
        }
        if loadout.missiles < MAX_LEVEL {
            options.push(Self::Missiles);
        }
        if options.is_empty() { Self::Repair } else { options[rng.below(options.len())] }
    }

    fn apply(self, loadout: &mut Loadout, player: &mut Player) {
        match self {
            Self::Laser => loadout.laser = (loadout.laser + 1).min(MAX_LEVEL),
            Self::Missiles => loadout.missiles = (loadout.missiles + 1).min(MAX_LEVEL),
            Self::Repair => player.health = (player.health + REPAIR_AMOUNT).min(MAX_HEALTH),
        }
    }
}

#[derive(Resource)]
pub struct ActiveTask {
    pub task: Task,
    purpose: Purpose,
    input: String,
    /// Verstrichene echte Zeit seit dem Öffnen.
    elapsed: f32,
    time_limit: f32,
    /// Verstrichene echte Zeit in der Rückkehr-Phase.
    returning: f32,
}

#[derive(Component)]
struct Popup;

#[derive(Component)]
struct PromptText;

#[derive(Component)]
struct TimeBar;

#[derive(Component)]
struct HintText;

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

fn count_freight(mut messages: MessageReader<FreightDestroyed>, mut pending: ResMut<PendingTasks>) {
    pending.freight += messages.read().count() as u32;
}

fn start_task(
    mut commands: Commands,
    mut pending: ResMut<PendingTasks>,
    mut topic: ResMut<Topic>,
    mut rng: ResMut<Rng>,
    loadout: Res<Loadout>,
    player: Single<&Player>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    let (purpose, extra_time) = if pending.super_weapon {
        pending.super_weapon = false;
        (Purpose::Super { second_chance_used: false }, SUPER_EXTRA_TIME)
    } else if pending.freight > 0 {
        pending.freight -= 1;
        (Purpose::Freight(Reward::choose(&mut rng, &loadout, player.health)), 0.0)
    } else {
        return;
    };
    let task = topic.0.next(&mut rng);
    let time_limit = task.time_limit + extra_time;
    commands.insert_resource(ActiveTask { task, purpose, input: String::new(), elapsed: 0.0, time_limit, returning: 0.0 });
    next.set(GamePhase::Task);
}

fn spawn_popup(mut commands: Commands, active: Res<ActiveTask>) {
    let root = commands
        .spawn((
            Popup,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(10.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                min_width: Val::Px(440.0),
                padding: UiRect::axes(Val::Px(32.0), Val::Px(20.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BorderColor::from(FRAME),
            BackgroundColor(Color::srgba(0.02, 0.04, 0.12, 0.88)),
            ChildOf(root),
        ))
        .id();
    let title = match &active.purpose {
        Purpose::Freight(reward) => format!("Fracht gesichert · Belohnung: {}", reward.name()),
        Purpose::Super { .. } => "Superwaffe laden · je schneller, desto stärker".to_string(),
    };
    commands.spawn((
        Text::new(title),
        TextColor(OKAY),
        font(18.0),
        ChildOf(panel),
    ));
    commands.spawn((
        PromptText,
        Text::new(format!("{} = _", active.task.prompt)),
        TextColor(TEXT),
        font(56.0),
        ChildOf(panel),
    ));
    let bar = commands
        .spawn((
            Node { width: Val::Px(380.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
            BackgroundColor(Color::srgb(0.08, 0.1, 0.2)),
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        TimeBar,
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
        BackgroundColor(GOOD),
        ChildOf(bar),
    ));
    commands.spawn((HintText, Text::new("Nummernblock · Enter"), TextColor(DIM), font(16.0), ChildOf(panel)));
}

impl ActiveTask {
    fn remaining_share(&self) -> f32 {
        (1.0 - self.elapsed / self.time_limit).clamp(0.0, 1.0)
    }
}

/// Ziffern über Nummernblock oder Ziffernreihe, Rücktaste löscht, Enter gibt ab.
fn read_digit(keys: &ButtonInput<KeyCode>) -> Option<char> {
    const KEYS: [(KeyCode, KeyCode); 10] = [
        (KeyCode::Digit0, KeyCode::Numpad0),
        (KeyCode::Digit1, KeyCode::Numpad1),
        (KeyCode::Digit2, KeyCode::Numpad2),
        (KeyCode::Digit3, KeyCode::Numpad3),
        (KeyCode::Digit4, KeyCode::Numpad4),
        (KeyCode::Digit5, KeyCode::Numpad5),
        (KeyCode::Digit6, KeyCode::Numpad6),
        (KeyCode::Digit7, KeyCode::Numpad7),
        (KeyCode::Digit8, KeyCode::Numpad8),
        (KeyCode::Digit9, KeyCode::Numpad9),
    ];
    KEYS.iter()
        .position(|(a, b)| keys.just_pressed(*a) || keys.just_pressed(*b))
        .and_then(|d| char::from_digit(d as u32, 10))
}

fn type_answer(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<ActiveTask>,
    mut topic: ResMut<Topic>,
    mut score: ResMut<Score>,
    mut loadout: ResMut<Loadout>,
    mut rng: ResMut<Rng>,
    mut fire: MessageWriter<FireSuper>,
    mut player: Single<&mut Player>,
    mut prompt: Single<(&mut Text, &mut TextColor), (With<PromptText>, Without<HintText>)>,
    mut hint: Single<(&mut Text, &mut TextColor), (With<HintText>, Without<PromptText>)>,
    mut bar: Single<(&mut Node, &mut BackgroundColor), With<TimeBar>>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    active.elapsed += time.delta_secs();
    if let Some(digit) = read_digit(&keys)
        && active.input.len() < MAX_DIGITS
    {
        active.input.push(digit);
    }
    if keys.just_pressed(KeyCode::Backspace) {
        active.input.pop();
    }

    let share = active.remaining_share();
    bar.0.width = Val::Percent(share * 100.0);
    bar.1.0 = if share > FAST_SHARE { GOOD } else if share > 0.2 { OKAY } else { BAD };
    prompt.0.0 = format!("{} = {}_", active.task.prompt, active.input);

    let submitted = (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && !active.input.is_empty();
    let outcome = if submitted {
        if active.input.parse::<u32>() == Ok(active.task.answer) {
            Outcome::Correct { remaining_share: share }
        } else {
            Outcome::Wrong
        }
    } else if share <= 0.0 {
        Outcome::TimedOut
    } else {
        return;
    };

    // Einzelne Felder getrennt ausleihen können.
    let active = &mut *active;
    let solution = format!("{} = {}", active.task.prompt, active.task.answer);
    topic.0.record(&active.task, &outcome);
    let (color, feedback) = match (&mut active.purpose, outcome) {
        (Purpose::Freight(reward), Outcome::Correct { remaining_share }) => {
            let points = (BASE_POINTS + SPEED_POINTS * remaining_share).round() as u32;
            score.0 += points;
            reward.apply(&mut loadout, &mut player);
            let praise = if remaining_share >= FAST_SHARE { "Blitzschnell!" } else { "Sauber gerechnet!" };
            (GOOD, format!("{praise} +{points} Punkte · {}", reward.name()))
        }
        (Purpose::Freight(reward), Outcome::TimedOut) => {
            reward.apply(&mut loadout, &mut player);
            (OKAY, format!("Zeit um – {} gibt's trotzdem. Nächstes Mal schneller!", reward.name()))
        }
        (Purpose::Freight(_), Outcome::Wrong) => {
            // Ein Fehler kostet etwas Schild, aber nie das Schiff.
            player.health = (player.health - WRONG_DAMAGE).max(1.0);
            (BAD, format!("Knapp daneben – richtig ist {}", active.task.answer))
        }
        (Purpose::Super { .. }, Outcome::Correct { remaining_share }) => {
            let points = (BASE_POINTS + SPEED_POINTS * remaining_share).round() as u32;
            score.0 += points;
            let fast = remaining_share >= FAST_SHARE;
            fire.write(FireSuper { tier: if fast { 3 } else { 2 }, power: remaining_share });
            let praise = if fast { "Volle Ladung!" } else { "Superwaffe gezündet!" };
            (GOOD, format!("{praise} +{points} Punkte"))
        }
        (Purpose::Super { second_chance_used }, Outcome::Wrong) if !*second_chance_used => {
            // Zweite Chance: neue Aufgabe, der Balken läuft einfach weiter.
            *second_chance_used = true;
            let message = format!("Falsch: {solution}. Neue Aufgabe, die Zeit läuft weiter!");
            active.task = topic.0.next(&mut rng);
            active.input.clear();
            hint.0.0 = message;
            hint.1.0 = BAD;
            return;
        }
        (Purpose::Super { .. }, Outcome::Wrong) => {
            player.health = (player.health - WRONG_DAMAGE).max(1.0);
            fire.write(FireSuper { tier: 1, power: 0.0 });
            (BAD, format!("Fehlzündung – richtig ist {}. Nur ein schwacher Puls.", active.task.answer))
        }
        (Purpose::Super { .. }, Outcome::TimedOut) => {
            fire.write(FireSuper { tier: 1, power: 0.0 });
            (OKAY, "Zeit um – ein schwacher Puls. Nächstes Mal schneller!".to_string())
        }
    };
    prompt.0.0 = solution;
    prompt.1.0 = color;
    hint.0.0 = feedback;
    hint.1.0 = color;
    next.set(GamePhase::Return);
}

fn finish_return(
    time: Res<Time<Real>>,
    mut active: ResMut<ActiveTask>,
    mut player: Single<&mut Player>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    active.returning += time.delta_secs();
    if active.returning >= RETURN_TIME {
        player.invulnerable = AFTER_RETURN_INVULNERABLE;
        next.set(GamePhase::Flying);
    }
}

fn close_popup(mut commands: Commands, popups: Query<Entity, With<Popup>>) {
    for popup in &popups {
        commands.entity(popup).despawn();
    }
    commands.remove_resource::<ActiveTask>();
}

/// Bremst die Welt beim Öffnen einer Aufgabe ab und beschleunigt sie danach sanft wieder.
fn world_speed(
    real: Res<Time<Real>>,
    phase: Res<State<GamePhase>>,
    active: Option<Res<ActiveTask>>,
    mut time: ResMut<Time<Virtual>>,
) {
    let speed = match (phase.get(), active) {
        (GamePhase::Task, _) => {
            let step = (1.0 - SLOW_MOTION) / SLOW_DOWN_TIME * real.delta_secs();
            (time.relative_speed() - step).max(SLOW_MOTION)
        }
        (GamePhase::Return, Some(active)) => {
            let t = (active.returning / RETURN_TIME).clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            SLOW_MOTION + (1.0 - SLOW_MOTION) * eased
        }
        _ => 1.0,
    };
    if speed != time.relative_speed() {
        time.set_relative_speed(speed);
    }
}
