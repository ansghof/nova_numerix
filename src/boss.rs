//! Der Boss am Levelende, „Der Wächter“: eine Festung mit vier Zahlenplatten vor dem Kern.
//! Oben steht eine Aufgabe; wer die Platte mit dem richtigen Ergebnis abschießt, öffnet den
//! Kern für ein paar Sekunden. Rechnen ist hier Zielen, die Welt läuft dabei in voller
//! Geschwindigkeit weiter.
//!
//! Auch hier gilt: falsch < Zeit abgelaufen < richtig < richtig und schnell. Läuft die Zeit
//! ab, zeigt der Boss die Lösung und öffnet den Kern kürzer; eine falsche Platte öffnet gar
//! nichts und löst einen gezielten Feuerstoß aus.

use bevy::prelude::*;

use crate::hud::Score;
use crate::level::{Rewind, BossDef, BossStyle, Campaign, CurrentLevel, LEVELS, LevelDef, LevelProgress, LevelStarted, LevelStats, star_string};
use crate::shipyard::{Credits, new_goods};
use crate::neon::{GOLD, GREEN, MAGENTA, ORANGE, RED, Shape, WHITE, polygon, scaled};
use crate::particles::Particles;
use crate::difficulty::Difficulty;
use crate::player::{Loadout, Player};
use crate::rng::Rng;
use crate::shots::{Friendly, Shot, Targetable, spawn_enemy_shot};
use crate::enemy::Enemy;
use crate::task::{BASE_POINTS, FAST_SHARE, Outcome, PendingTasks, SPEED_POINTS, TASK_CREDITS, Task, Topic, binary_digits, from_binary_digits};
use crate::{AppState, Gameplay, Playfield, Shake};

pub const BOSS_HEALTH: f32 = 150.0;
/// So weit vom rechten Rand bleibt der Boss stehen, nachdem er hereingeflogen ist.
const HOLD_FROM_EDGE: f32 = 190.0;
const ENTER_SPEED: f32 = 140.0;
const BOB_HEIGHT: f32 = 80.0;
const PLATE_X: f32 = -135.0;
const PLATE_YS: [f32; 4] = [-170.0, -88.0, 88.0, 170.0];
const PLATE_RADIUS: f32 = 30.0;
const CORE_POS: Vec2 = Vec2::new(-80.0, 0.0);
const CORE_RADIUS: f32 = 32.0;
/// Schüsse, die weiter rechts als dieser Versatz ankommen, prallen am Rumpf ab.
const BODY_FRONT: f32 = -105.0;
const BODY_HALF_HEIGHT: f32 = 200.0;
/// Zielen dauert länger als Tippen, deshalb mehr Zeit als im Aufgabenfenster.
const AIM_TIME_FACTOR: f32 = 1.6;
const OPEN_TIME: f32 = 6.0;
const TIMEOUT_OPEN_TIME: f32 = 3.0;
const WRONG_PAUSE: f32 = 1.8;
const FIRST_PAUSE: f32 = 2.0;
const DEATH_TIME: f32 = 2.4;
const BOSS_POINTS: u32 = 5000;
const BOSS_CREDITS: u32 = 150;
const SHOT_SPEED: f32 = 240.0;
const BURST_SPEED: f32 = 360.0;
/// Die Superwaffe wirkt auf den Boss so viel stärker als auf normale Gegner.
pub const SUPER_MULTIPLIER: f32 = 4.0;

const TEXT: Color = Color::srgb(0.92, 0.96, 1.0);
const DIM: Color = Color::srgb(0.08, 0.1, 0.2);
const GOOD: Color = Color::srgb(0.45, 1.0, 0.6);
const OKAY: Color = Color::srgb(1.0, 0.85, 0.35);
const BAD: Color = Color::srgb(1.0, 0.4, 0.4);

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossState>()
            .add_systems(
                Update,
                (spawn_boss, move_boss, run_round, hit_boss, attack, animate, update_banner, dying, victory)
                    .chain()
                    .in_set(Gameplay),
            )
            .add_systems(Update, clear_on_start);
    }
}

#[derive(Resource, Default)]
pub struct BossState {
    spawned: bool,
    pub defeated: bool,
    /// Den letzten Schlag hat eine volle Superwaffen-Ladung (Stufe 3) gesetzt: dritter Stern.
    pub super_finish: bool,
}

enum Round {
    /// Die Aufgabe steht oben, die Platten warten auf einen Treffer.
    Asking { elapsed: f32, limit: f32 },
    /// Der Kern ist offen.
    Open { remaining: f32, total: f32 },
    /// Kurze Pause, etwa nach einer falschen Platte.
    Pause { remaining: f32 },
}

#[derive(Component)]
pub struct Boss {
    pub health: f32,
    round: Round,
    task: Option<Task>,
    /// Rückmeldung zur letzten Runde für das Banner.
    feedback: (String, Color),
    age: f32,
    entered: bool,
    volley_timer: f32,
    volleys: u32,
    dying: Option<f32>,
    /// Der Treffer, der ihn erledigt hat, kam von einer vollen Superwaffen-Ladung.
    pub super_finish: bool,
}

impl Boss {
    pub fn alive(&self) -> bool {
        self.dying.is_none()
    }

    pub fn asking(&self) -> bool {
        matches!(self.round, Round::Asking { .. })
    }

    pub fn core_open(&self) -> bool {
        matches!(self.round, Round::Open { .. })
    }
}

#[derive(Component)]
pub struct Plate {
    value: u32,
    pub correct: bool,
    opened: bool,
    /// Animation von geschlossen (0) zu offen (1).
    open: f32,
    /// Rotes Aufblinken nach einem falschen Treffer.
    red: f32,
    label: Entity,
}

#[derive(Component)]
pub struct BossCore {
    flash: f32,
}

#[derive(Component)]
struct Banner;

#[derive(Component)]
struct BannerText;

#[derive(Component)]
struct BannerBar;

#[derive(Component)]
struct BossHealthFill;

#[derive(Component)]
struct VictoryScreen;

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

/// Punkte auf einem Kreisbogen, für Wirbel und runde Rümpfe.
fn arc(center: Vec2, radius: f32, from: f32, to: f32, steps: usize) -> Vec<Vec2> {
    (0..=steps).map(|i| center + Vec2::from_angle(from + (to - from) * i as f32 / steps as f32) * radius).collect()
}

fn body_shape(def: &BossDef) -> Shape {
    let core_ring = polygon(6, 40.0, 0.0).iter().map(|p| *p + CORE_POS).collect::<Vec<_>>();
    if def.style == BossStyle::Storm {
        // Eine runde Festung, um die Sturmbänder wirbeln.
        let center = Vec2::new(60.0, 0.0);
        let mut shape = Shape::new(def.color).closed(&polygon(14, 172.0, 0.0).iter().map(|p| *p + center).collect::<Vec<_>>());
        for i in 0..4 {
            let start = i as f32 * std::f32::consts::FRAC_PI_2;
            shape = shape.open(&arc(center, 120.0, start, start + 1.2, 8)).open(&arc(center, 205.0, start + 0.6, start + 1.5, 8));
        }
        return shape.closed(&core_ring);
    }
    if def.style == BossStyle::Processor {
        // Ein Chip: Gehäuse, Kern in der Mitte und Anschlussbeinchen oben, unten und hinten.
        let mut shape = Shape::new(def.color)
            .closed(&[Vec2::new(-110.0, -190.0), Vec2::new(130.0, -190.0), Vec2::new(130.0, 190.0), Vec2::new(-110.0, 190.0)])
            .closed(&[Vec2::new(-20.0, -90.0), Vec2::new(90.0, -90.0), Vec2::new(90.0, 90.0), Vec2::new(-20.0, 90.0)]);
        for i in 0..7 {
            let x = -80.0 + i as f32 * 33.0;
            shape = shape.open(&[Vec2::new(x, 190.0), Vec2::new(x, 218.0)]).open(&[Vec2::new(x, -190.0), Vec2::new(x, -218.0)]);
        }
        for i in 0..9 {
            let y = -160.0 + i as f32 * 40.0;
            shape = shape.open(&[Vec2::new(130.0, y), Vec2::new(158.0, y)]);
        }
        return shape.closed(&core_ring);
    }
    if def.style == BossStyle::Cube {
        // Ein Würfel, schräg von vorn gesehen, mit Gitterlinien auf der Vorderseite.
        return Shape::new(def.color)
            .closed(&[Vec2::new(-110.0, -190.0), Vec2::new(110.0, -190.0), Vec2::new(110.0, 190.0), Vec2::new(-110.0, 190.0)])
            .open(&[Vec2::new(-110.0, 190.0), Vec2::new(-60.0, 222.0), Vec2::new(160.0, 222.0), Vec2::new(160.0, -158.0), Vec2::new(110.0, -190.0)])
            .open(&[Vec2::new(110.0, 190.0), Vec2::new(160.0, 222.0)])
            .open(&[Vec2::new(37.0, -190.0), Vec2::new(37.0, 190.0)])
            .open(&[Vec2::new(-30.0, -63.0), Vec2::new(110.0, -63.0)])
            .open(&[Vec2::new(-30.0, 63.0), Vec2::new(110.0, 63.0)])
            .closed(&core_ring);
    }
    Shape::new(def.color)
        .closed(&[
            Vec2::new(-105.0, -110.0),
            Vec2::new(-60.0, -205.0),
            Vec2::new(60.0, -215.0),
            Vec2::new(140.0, -130.0),
            Vec2::new(160.0, 0.0),
            Vec2::new(140.0, 130.0),
            Vec2::new(60.0, 215.0),
            Vec2::new(-60.0, 205.0),
            Vec2::new(-105.0, 110.0),
            Vec2::new(-115.0, 40.0),
            Vec2::new(-115.0, -40.0),
        ])
        .open(&[Vec2::new(-30.0, -180.0), Vec2::new(70.0, -120.0), Vec2::new(70.0, 120.0), Vec2::new(-30.0, 180.0)])
        .open(&[Vec2::new(70.0, 0.0), Vec2::new(150.0, 0.0)])
        .closed(&core_ring)
}

fn plate_shape() -> Shape {
    Shape::new(GOLD).closed(&[
        Vec2::new(-26.0, -24.0),
        Vec2::new(22.0, -24.0),
        Vec2::new(30.0, 0.0),
        Vec2::new(22.0, 24.0),
        Vec2::new(-26.0, 24.0),
    ])
}

fn core_shape() -> Shape {
    Shape::new(MAGENTA).closed(&polygon(8, 24.0, 0.0)).closed(&polygon(4, 10.0, 0.0))
}

fn spawn_boss(
    mut commands: Commands,
    progress: Res<LevelProgress>,
    mut state: ResMut<BossState>,
    mut shake: ResMut<Shake>,
    field: Res<Playfield>,
    current: Res<CurrentLevel>,
) {
    if state.spawned || !progress.boss_ahead() {
        return;
    }
    state.spawned = true;
    shake.0 = 6.0;
    let def = &current.def().boss;
    let boss = commands
        .spawn((
            Boss {
                health: BOSS_HEALTH,
                round: Round::Pause { remaining: FIRST_PAUSE },
                task: None,
                feedback: (format!("Achtung: {}!", def.name), BAD),
                age: 0.0,
                entered: false,
                volley_timer: 3.0,
                volleys: 0,
                dying: None,
                super_finish: false,
            },
            body_shape(def),
            Transform::from_xyz(field.half_width + 280.0, 0.0, 0.0),
            // Die Zahlen auf den Platten brauchen eine durchgehende Sichtbarkeits-Kette.
            Visibility::default(),
        ))
        .id();
    for y in PLATE_YS {
        let label = commands
            .spawn((
                Text2d::new(""),
                font(30.0),
                TextColor(TEXT),
                Transform::from_xyz(0.0, 0.0, 0.1),
                Visibility::Hidden,
            ))
            .id();
        let plate = commands
            .spawn((
                Plate { value: 0, correct: false, opened: false, open: 0.0, red: 0.0, label },
                plate_shape(),
                Transform::from_xyz(PLATE_X, y, 0.2),
                Visibility::default(),
                ChildOf(boss),
            ))
            .id();
        commands.entity(label).insert(ChildOf(plate));
    }
    commands.spawn((BossCore { flash: 0.0 }, core_shape(), Transform::from_translation(CORE_POS.extend(0.2)), ChildOf(boss)));
    spawn_banner(&mut commands, def.name);
}

fn spawn_banner(commands: &mut Commands, name: &str) {
    let root = commands
        .spawn((
            Banner,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Px(40.0),
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
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.12, 0.75)),
            ChildOf(root),
        ))
        .id();
    commands.spawn((Text::new(name), TextColor(BAD), font(14.0), ChildOf(panel)));
    let health = commands
        .spawn((
            Node { width: Val::Px(320.0), height: Val::Px(6.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(DIM),
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        BossHealthFill,
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
        BackgroundColor(BAD),
        ChildOf(health),
    ));
    commands.spawn((BannerText, Text::new(""), TextColor(TEXT), font(26.0), ChildOf(panel)));
    let bar = commands
        .spawn((
            Node { width: Val::Px(320.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(DIM),
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        BannerBar,
        Node { width: Val::Percent(0.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
        BackgroundColor(GOOD),
        ChildOf(bar),
    ));
}

fn move_boss(
    time: Res<Time>,
    field: Res<Playfield>,
    boss: Single<(&mut Boss, &mut Transform), Without<Player>>,
    player: Single<&mut Transform, With<Player>>,
) {
    let (mut boss, mut transform) = boss.into_inner();
    let dt = time.delta_secs();
    let hold_x = field.half_width - HOLD_FROM_EDGE;
    if transform.translation.x > hold_x {
        transform.translation.x = (transform.translation.x - ENTER_SPEED * dt).max(hold_x);
    } else {
        boss.entered = true;
        boss.age += dt;
        transform.translation.y = (boss.age * 0.5).sin() * BOB_HEIGHT;
    }
    // Durch den Rumpf kann man nicht hindurchfliegen.
    let mut player = player.into_inner();
    let front = transform.translation.x + BODY_FRONT - 30.0;
    if player.translation.x > front && (player.translation.y - transform.translation.y).abs() < BODY_HALF_HEIGHT {
        player.translation.x = front;
    }
}

/// Drei falsche, aber plausible Binärzahlen: dieselbe Zahl mit einem umgekippten Bit.
fn binary_distractors(answer: u32, rng: &mut Rng) -> Vec<u32> {
    let n = from_binary_digits(answer).unwrap_or(1);
    let mut out = Vec::new();
    for _ in 0..100 {
        if out.len() == 3 {
            break;
        }
        let candidate = n ^ (1 << rng.below(4));
        let digits = binary_digits(candidate);
        if candidate > 0 && candidate != n && !out.contains(&digits) {
            out.push(digits);
        }
    }
    out
}

/// Drei falsche, aber plausible Ergebnisse: Nachbarn, Zehnersprünge, vertauschte Ziffern.
fn distractors(answer: u32, rng: &mut Rng) -> Vec<u32> {
    const OFFSETS: [u32; 9] = [1, 2, 3, 4, 5, 6, 8, 10, 12];
    let mut out = Vec::new();
    let reversed: u32 = answer.to_string().chars().rev().collect::<String>().parse().unwrap_or(0);
    if answer >= 10 && reversed != answer && reversed > 0 && rng.chance(0.6) {
        out.push(reversed);
    }
    for _ in 0..100 {
        if out.len() == 3 {
            break;
        }
        let offset = OFFSETS[rng.below(OFFSETS.len())];
        let candidate = if rng.chance(0.5) { answer + offset } else { answer.saturating_sub(offset) };
        if candidate > 0 && candidate != answer && !out.contains(&candidate) {
            out.push(candidate);
        }
    }
    out
}

fn new_round(boss: &mut Boss, plates: &mut Query<&mut Plate>, topic: &mut Topic, rng: &mut Rng) {
    let task = topic.next(rng);
    let mut values = if task.binary { binary_distractors(task.answer, rng) } else { distractors(task.answer, rng) };
    values.push(task.answer);
    for i in (1..values.len()).rev() {
        values.swap(i, rng.below(i + 1));
    }
    for (mut plate, value) in plates.iter_mut().zip(values) {
        plate.value = value;
        plate.correct = value == task.answer;
        plate.opened = false;
    }
    let limit = task.time_limit * AIM_TIME_FACTOR;
    boss.task = Some(task);
    boss.round = Round::Asking { elapsed: 0.0, limit };
}

fn run_round(
    mut commands: Commands,
    time: Res<Time>,
    mut topic: ResMut<Topic>,
    mut rng: ResMut<Rng>,
    mut stats: ResMut<LevelStats>,
    mut boss: Single<&mut Boss>,
    mut plates: Query<&mut Plate>,
    core: Single<Entity, With<BossCore>>,
) {
    if !boss.entered || !boss.alive() {
        return;
    }
    let dt = time.delta_secs();
    let boss = &mut *boss;
    match &mut boss.round {
        Round::Asking { elapsed, limit } => {
            *elapsed += dt;
            if *elapsed < *limit {
                return;
            }
            // Zeit um: Lösung zeigen und den Kern kürzer öffnen, ohne Punkte.
            let task = boss.task.as_ref().unwrap();
            topic.0.record(task, &Outcome::TimedOut);
            stats.count(&Outcome::TimedOut);
            boss.feedback = (format!("Zeit um: {} · Kern kurz offen!", task.solution()), OKAY);
            for mut plate in &mut plates {
                plate.opened = plate.correct;
            }
            boss.round = Round::Open { remaining: TIMEOUT_OPEN_TIME, total: TIMEOUT_OPEN_TIME };
            commands.entity(*core).insert(Targetable);
        }
        Round::Open { remaining, .. } => {
            *remaining -= dt;
            if *remaining <= 0.0 {
                commands.entity(*core).remove::<Targetable>();
                new_round(boss, &mut plates, &mut topic, &mut rng);
            }
        }
        Round::Pause { remaining } => {
            *remaining -= dt;
            if *remaining <= 0.0 {
                new_round(boss, &mut plates, &mut topic, &mut rng);
            }
        }
    }
}

fn hit_boss(
    mut commands: Commands,
    mut topic: ResMut<Topic>,
    mut stats: ResMut<LevelStats>,
    mut score: ResMut<Score>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut credits: ResMut<Credits>,
    shots: Query<(Entity, &Shot, &Transform), With<Friendly>>,
    boss: Single<(&mut Boss, &Transform)>,
    mut plates: Query<(&mut Plate, &GlobalTransform)>,
    core: Single<(Entity, &mut BossCore, &GlobalTransform)>,
    player: Single<&Transform, (With<Player>, Without<Boss>)>,
) {
    let (mut boss, boss_transform) = boss.into_inner();
    let (core_entity, mut core, core_transform) = core.into_inner();
    if !boss.alive() {
        return;
    }
    let boss_pos = boss_transform.translation.truncate();
    let core_pos = core_transform.translation().truncate();

    for (shot_entity, shot, transform) in &shots {
        let pos = transform.translation.truncate();

        // Zahlenplatten
        let hit = plates.iter_mut().find(|(plate, plate_transform)| {
            plate.open < 0.5 && plate_transform.translation().truncate().distance(pos) < PLATE_RADIUS + shot.radius
        });
        if let Some((mut plate, _)) = hit {
            commands.entity(shot_entity).try_despawn();
            particles.sparks(&mut rng, pos, GOLD, 4, 180.0);
            let Round::Asking { elapsed, limit } = boss.round else {
                continue;
            };
            let task = boss.task.as_ref().unwrap();
            if plate.correct {
                let share = (1.0 - elapsed / limit).clamp(0.0, 1.0);
                let outcome = Outcome::Correct { remaining_share: share };
                topic.0.record(task, &outcome);
                stats.count(&outcome);
                let points = (BASE_POINTS + SPEED_POINTS * share).round() as u32;
                score.0 += points;
                let praise = if share >= FAST_SHARE { "Blitzschnell!" } else { "Treffer!" };
                boss.feedback = (format!("{praise} +{points} · Kern offen – Feuer!"), GOOD);
                credits.flight += TASK_CREDITS;
                plate.opened = true;
                boss.round = Round::Open { remaining: OPEN_TIME, total: OPEN_TIME };
                commands.entity(core_entity).insert(Targetable);
            } else {
                topic.0.record(task, &Outcome::Wrong);
                stats.count(&Outcome::Wrong);
                boss.feedback = (format!("Falsch: {}", task.solution()), BAD);
                plate.red = 1.0;
                boss.round = Round::Pause { remaining: WRONG_PAUSE };
                // Strafe: ein gezielter Feuerstoß aus dem Kern.
                let dir = (player.translation.truncate() - core_pos).normalize_or(Vec2::NEG_X);
                for angle in [-0.12, 0.0, 0.12] {
                    spawn_enemy_shot(&mut commands, core_pos, Vec2::from_angle(angle).rotate(dir) * BURST_SPEED);
                }
            }
            continue;
        }

        // Kern
        if boss.core_open() && pos.distance(core_pos) < CORE_RADIUS + shot.radius {
            commands.entity(shot_entity).try_despawn();
            boss.health -= shot.damage;
            core.flash = 1.0;
            particles.sparks(&mut rng, pos, GREEN, 6, 260.0);
            continue;
        }

        // Rumpf
        if pos.x > boss_pos.x + BODY_FRONT && (pos.y - boss_pos.y).abs() < BODY_HALF_HEIGHT {
            commands.entity(shot_entity).try_despawn();
            particles.sparks(&mut rng, pos, scaled(MAGENTA, 0.6), 3, 120.0);
        }
    }
}

fn attack(
    mut commands: Commands,
    time: Res<Time>,
    boss: Single<(&mut Boss, &Transform)>,
    core: Single<&GlobalTransform, With<BossCore>>,
    player: Single<&Transform, (With<Player>, Without<Boss>)>,
    current: Res<CurrentLevel>,
) {
    let (mut boss, _) = boss.into_inner();
    if !boss.entered || !boss.alive() {
        return;
    }
    boss.volley_timer -= time.delta_secs();
    if boss.volley_timer > 0.0 {
        return;
    }
    // Angeschlagen wird der Wächter wütender.
    let enraged = boss.health < BOSS_HEALTH * 0.5;
    boss.volley_timer = if enraged { 1.7 } else { 2.6 };
    boss.volleys += 1;
    let from = core.translation().truncate();
    // Zweite Phase der Sturmfestung: jede zweite Salve ein drehender Ring aus Schüssen.
    if enraged && current.def().boss.style == BossStyle::Storm && boss.volleys % 2 == 0 {
        let turn = boss.volleys as f32 * 0.35;
        for i in 0..10 {
            let angle = turn + i as f32 / 10.0 * std::f32::consts::TAU;
            spawn_enemy_shot(&mut commands, from, Vec2::from_angle(angle) * SHOT_SPEED * 0.9);
        }
        return;
    }
    if boss.volleys % 3 == 0 {
        // Gezielte Dreierreihe
        let dir = (player.translation.truncate() - from).normalize_or(Vec2::NEG_X);
        for i in 0..3 {
            spawn_enemy_shot(&mut commands, from - dir * i as f32 * 22.0, dir * BURST_SPEED);
        }
    } else {
        // Fächer nach links
        let count = if enraged { 7 } else { 5 };
        for i in 0..count {
            let spread = (i as f32 / (count - 1) as f32 - 0.5) * 1.1;
            spawn_enemy_shot(&mut commands, from, Vec2::from_angle(std::f32::consts::PI + spread) * SHOT_SPEED);
        }
    }
}

fn animate(
    time: Res<Time>,
    real: Res<Time<Real>>,
    boss: Single<&Boss>,
    mut plates: Query<(&mut Plate, &mut Transform, &mut Shape)>,
    mut labels: Query<(&mut Text2d, &mut TextFont, &mut Visibility)>,
    core: Single<(&mut BossCore, &mut Shape), Without<Plate>>,
) {
    let dt = time.delta_secs();
    let pulse = 0.75 + 0.25 * (real.elapsed_secs() * 8.0).sin();
    let pause = matches!(boss.round, Round::Pause { .. }) && boss.task.is_some();
    for (mut plate, mut transform, mut shape) in &mut plates {
        let target = if plate.opened { 1.0 } else { 0.0 };
        plate.open += (target - plate.open).clamp(-4.0 * dt, 4.0 * dt);
        plate.red = (plate.red - dt * 2.0).max(0.0);
        // Eine offene Platte gleitet zur Seite und schrumpft.
        transform.translation.x = PLATE_X + plate.open * 45.0;
        transform.scale = Vec3::new(1.0, 1.0 - plate.open * 0.7, 1.0);
        let base = if pause && plate.correct { scaled(GREEN, pulse) } else { GOLD };
        shape.color = scaled(base, 1.0 - plate.open * 0.7).mix(&RED, plate.red);
        if let Ok((mut text, mut font, mut visibility)) = labels.get_mut(plate.label) {
            text.0 = boss.task.as_ref().map(|t| t.show(plate.value)).unwrap_or_default();
            // Vier Binärziffern brauchen eine kleinere Schrift, um auf die Platte zu passen.
            let size = FontSize::Px(if text.0.len() > 3 { 20.0 } else { 30.0 });
            if font.font_size != size {
                font.font_size = size;
            }
            let show = plate.value > 0 && plate.open < 0.5 && !boss.core_open();
            *visibility = if show { Visibility::Inherited } else { Visibility::Hidden };
        }
    }
    let (mut core, mut shape) = core.into_inner();
    core.flash = (core.flash - dt * 8.0).max(0.0);
    let base = if boss.core_open() { scaled(GREEN, pulse * 1.3) } else { scaled(MAGENTA, 0.35) };
    shape.color = base.mix(&WHITE, core.flash);
}

fn update_banner(
    boss: Single<&Boss>,
    mut text: Single<(&mut Text, &mut TextColor), With<BannerText>>,
    mut bar: Single<(&mut Node, &mut BackgroundColor), (With<BannerBar>, Without<BossHealthFill>)>,
    mut health: Single<&mut Node, (With<BossHealthFill>, Without<BannerBar>)>,
) {
    health.width = Val::Percent((boss.health / BOSS_HEALTH).clamp(0.0, 1.0) * 100.0);
    let (message, color, share, bar_color) = match &boss.round {
        Round::Asking { elapsed, limit } => {
            let share = (1.0 - elapsed / limit).clamp(0.0, 1.0);
            let prompt = boss.task.as_ref().map(|t| t.prompt.as_str()).unwrap_or("");
            let hint = boss.task.as_ref().and_then(|t| t.hint).map(|h| format!("  ({h})")).unwrap_or_default();
            let bar_color = if share > FAST_SHARE { GOOD } else if share > 0.2 { OKAY } else { BAD };
            (format!("Triff die richtige Zahl: {prompt}{hint}"), TEXT, share, bar_color)
        }
        Round::Open { remaining, total } => (boss.feedback.0.clone(), boss.feedback.1, remaining / total, GOOD),
        Round::Pause { .. } => (boss.feedback.0.clone(), boss.feedback.1, 0.0, GOOD),
    };
    text.0.0 = message;
    text.1.0 = color;
    bar.0.width = Val::Percent(share * 100.0);
    bar.1.0 = bar_color;
}

/// Der Untergang: eine Kette von Explosionen über den Rumpf, dann ein großer Knall.
fn dying(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<Shake>,
    mut score: ResMut<Score>,
    mut state: ResMut<BossState>,
    mut pending: ResMut<PendingTasks>,
    boss: Single<(Entity, &mut Boss, &Transform, &Shape)>,
    banner: Query<Entity, With<Banner>>,
    shots: Query<Entity, With<Shot>>,
    enemies: Query<(Entity, &Shape, &Transform), (With<Enemy>, Without<Boss>)>,
) {
    let (entity, mut boss, transform, shape) = boss.into_inner();
    if boss.health > 0.0 {
        return;
    }
    let t = boss.dying.get_or_insert(0.0);
    let before = *t;
    *t += time.delta_secs();
    let now = *t;
    let center = transform.translation.truncate();

    // Alle 0,15 Sekunden eine Explosion irgendwo im Rumpf.
    if (now / 0.15) as u32 != (before / 0.15) as u32 {
        let at = center + Vec2::new(rng.range(-110.0, 150.0), rng.range(-190.0, 190.0));
        let color = [MAGENTA, ORANGE, WHITE][rng.below(3)];
        particles.sparks(&mut rng, at, color, 26, 420.0);
        particles.ring(at, color, 300.0, 0.3);
        shake.0 = shake.0.max(6.0);
    }
    if now < DEATH_TIME {
        return;
    }

    particles.explode(&mut rng, shape, transform, 3.0);
    particles.ring(center, WHITE, 900.0, 0.8);
    shake.0 = 22.0;
    score.0 += BOSS_POINTS;
    state.defeated = true;
    state.super_finish = boss.super_finish;
    info!("Boss besiegt{}", if boss.super_finish { " mit voller Superwaffe" } else { "" });
    commands.entity(entity).despawn();
    for banner in &banner {
        commands.entity(banner).despawn();
    }
    for shot in &shots {
        commands.entity(shot).try_despawn();
    }
    // Was noch herumfliegt, geht in der Druckwelle mit unter; ohne Aufgabe für Frachtdrohnen.
    for (enemy, enemy_shape, enemy_transform) in &enemies {
        particles.explode(&mut rng, enemy_shape, enemy_transform, 1.0);
        commands.entity(enemy).despawn();
    }
    pending.clear();
}

/// Was der Siegerbildschirm über Sterne und Schwierigkeit wissen muss.
struct Result {
    stars: [bool; 3],
    percent: u32,
    best: u8,
    difficulty: Difficulty,
}

fn spawn_victory(
    commands: &mut Commands,
    name: &str,
    score: u32,
    earned: u32,
    stats: &LevelStats,
    loadout: &Loadout,
    unlocked: Option<&LevelDef>,
    goods: Option<&str>,
    result: &Result,
) {
    let root = commands
        .spawn((
            VictoryScreen,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.05, 0.55)),
            GlobalZIndex(10),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(40.0), Val::Px(28.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BorderColor::from(GOOD),
            BackgroundColor(Color::srgba(0.02, 0.04, 0.12, 0.92)),
            ChildOf(root),
        ))
        .id();
    let weapons = loadout.describe();
    let lines = [
        ("Level geschafft!".to_string(), GOOD, 44.0),
        (format!("{name} ist zerstört."), TEXT, 20.0),
        (format!("{score} Punkte · +{earned} Credits"), OKAY, 28.0),
        (
            format!(
                "Aufgaben: {} richtig ({} blitzschnell) · {} Zeit um · {} falsch",
                stats.correct, stats.fast, stats.timed_out, stats.wrong
            ),
            TEXT,
            18.0,
        ),
        (format!("Bleibt an Bord: {weapons}"), TEXT, 18.0),
    ];
    for (text, color, size) in lines {
        commands.spawn((Text::new(text), TextColor(color), font(size), ChildOf(panel)));
    }
    // Die drei Sterne mit Erklärung, damit man weiß, wofür es den nächsten gibt.
    let star_lines = [
        "Level geschafft".to_string(),
        format!("Mindestens 80 % richtig gerechnet (diesmal {} %)", result.percent),
        if result.stars[2] {
            "Den Boss mit voller Superwaffen-Ladung erledigt".to_string()
        } else {
            "Den Boss mit voller Superwaffen-Ladung erledigen (Tipp: eine Ladung aufheben und blitzschnell rechnen)".to_string()
        },
    ];
    for (earned, line) in result.stars.iter().zip(star_lines) {
        let (mark, color) = if *earned { ('★', OKAY) } else { ('☆', Color::srgb(0.55, 0.62, 0.75)) };
        commands.spawn((Text::new(format!("{mark} {line}")), TextColor(color), font(17.0), ChildOf(panel)));
    }
    let best = format!("Bestwert für dieses Level: {} · Schwierigkeit: {}", star_string(result.best), result.difficulty.name());
    commands.spawn((Text::new(best), TextColor(TEXT), font(15.0), ChildOf(panel)));
    if let Some(level) = unlocked {
        let message = format!("Neues Level freigeschaltet: {} · {}", level.world, level.topic);
        commands.spawn((Text::new(message), TextColor(GOOD), font(18.0), ChildOf(panel)));
    }
    if let Some(goods) = goods {
        let message = format!("Quibbo hat neue Ware im Laden: {goods}");
        commands.spawn((Text::new(message), TextColor(GOOD), font(18.0), ChildOf(panel)));
    }
    commands.spawn((Text::new("Enter: zur Werft"), TextColor(Color::srgb(0.55, 0.62, 0.75)), font(16.0), ChildOf(panel)));
}

/// Nach dem Sieg: Credits des Flugs aufs Konto, nächstes Level freischalten, Auswertung zeigen.
/// Enter führt in die Werft.
fn victory(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    real: Res<Time<Real>>,
    mut time: ResMut<Time<Virtual>>,
    state: Res<BossState>,
    score: Res<Score>,
    stats: Res<LevelStats>,
    loadout: Res<Loadout>,
    mut credits: ResMut<Credits>,
    mut campaign: ResMut<Campaign>,
    current: Res<CurrentLevel>,
    difficulty: Res<Difficulty>,
    mut next: ResMut<NextState<AppState>>,
    mut waited: Local<f32>,
    mut shown: Local<bool>,
) {
    if !state.defeated {
        *waited = 0.0;
        *shown = false;
        return;
    }
    if !*shown {
        *shown = true;
        // Was an Checkpoints schon gesichert wurde, zählt zur Ausbeute des Levels dazu.
        let earned = credits.secured + credits.flight + BOSS_CREDITS;
        credits.bank += credits.flight + BOSS_CREDITS;
        credits.flight = 0;
        let unlocked_before = campaign.unlocked;
        campaign.unlocked = campaign.unlocked.max((current.0 + 2).min(LEVELS.len()));
        let unlocked = (campaign.unlocked > unlocked_before).then(|| &LEVELS[campaign.unlocked - 1]);
        let beaten_before = campaign.beaten;
        campaign.beaten = campaign.beaten.max(current.0 + 1);
        let goods = (campaign.beaten > beaten_before).then(|| new_goods(campaign.beaten)).flatten();
        // Sterne: geschafft, mindestens 80 % richtig, Boss mit voller Superwaffe erledigt.
        let total = stats.correct + stats.timed_out + stats.wrong;
        let percent = if total > 0 { stats.correct * 100 / total } else { 0 };
        let earned_stars = [true, total > 0 && percent >= 80, state.super_finish];
        let mask = earned_stars.iter().enumerate().fold(0u8, |m, (i, s)| if *s { m | 1 << i } else { m });
        if let Some(best) = campaign.stars.get_mut(current.0) {
            *best |= mask;
        }
        info!("Sterne: {}", star_string(mask));
        let result = Result { stars: earned_stars, percent, best: campaign.stars_of(current.0), difficulty: *difficulty };
        spawn_victory(&mut commands, current.def().boss.name, score.0, earned, &stats, &loadout, unlocked, goods, &result);
    }
    // Nach dem großen Knall die Welt anhalten, bis es weitergeht.
    *waited += real.delta_secs();
    if *waited > 1.5 && !time.is_paused() {
        time.pause();
    }
    if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
        next.set(AppState::Shipyard);
    }
}

/// Ein neues Level beginnt: Boss, Banner und Siegerbild verschwinden.
/// Ein neues Level oder ein Neustart am Checkpoint: Boss, Banner und Siegerbild verschwinden.
/// Steht der Boss danach noch bevor, kommt er frisch wieder.
fn clear_on_start(
    mut commands: Commands,
    mut started: MessageReader<LevelStarted>,
    mut rewind: MessageReader<Rewind>,
    mut state: ResMut<BossState>,
    leftovers: Query<Entity, Or<(With<Boss>, With<Banner>, With<VictoryScreen>)>>,
) {
    if started.read().count() + rewind.read().count() == 0 {
        return;
    }
    *state = BossState::default();
    for entity in &leftovers {
        commands.entity(entity).despawn();
    }
}
