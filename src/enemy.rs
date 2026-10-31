//! Gegnerwellen, die von rechts hereinfliegen, und die goldenen Frachtdrohnen, deren Abschuss
//! eine Aufgabe auslöst. Welche Formationen kommen, bestimmt das Level: Im Neonnebel fliegen
//! Wackler, Pfeile und Kreisel, auf der Gitterstation warten Geschütze am Rand, Teiler, die in
//! Splitter zerfallen, und Sucher, die dem Schiff nachsteuern. Am Gasriesen treiben Minen, die
//! hochgehen, wenn man ihnen zu nahe kommt, und Kreuzer feuern Fächer. Im Datenkern sind Bits
//! nur verwundbar, solange sie eine 1 zeigen, und Datenpakete rasen in Strömen vorbei.
//!
//! Jede Frachtdrohne zeigt in ihrem Inneren und auf einem Schild darunter, was sie geladen hat.

use bevy::prelude::*;

use crate::hud::Score;
use crate::neon::{GOLD, GREEN, LIME, MAGENTA, ORANGE, PINK, RED, Shape, VIOLET, WHITE, polygon, scaled};
use crate::particles::Particles;
use crate::player::{BUBBLE_RADIUS, HIT_RADIUS, Loadout, Player};
use crate::rng::Rng;
use crate::level::{Campaign, CurrentLevel, LevelProgress, LevelStarted, Rewind};
use crate::difficulty::Difficulty;
use crate::ship::Ship;
use crate::task::Reward;
use crate::shipyard::Credits;
use crate::shots::{Targetable, spawn_enemy_shot};
use crate::sound::play;
use crate::synth::Sfx;
use crate::{GamePhase, Gameplay, HALF_HEIGHT, Playfield, Shake};

/// So weit rechts außerhalb des sichtbaren Felds tauchen Gegner auf.
const SPAWN_MARGIN: f32 = 60.0;
const RAM_DAMAGE: f32 = 25.0;
const ENEMY_SHOT_SPEED: f32 = 260.0;
/// Die erste Frachtdrohne kommt früh, damit man die Aufgaben gleich kennenlernt.
const FIRST_FREIGHTER: f32 = 6.0;
const DART_SPEED: f32 = 380.0;
/// Wie schnell ein Kreisel-Verband nach links zieht und sich dabei dreht.
const RING_SPEED: f32 = 120.0;
const RING_SPIN: f32 = 1.8;
/// Geschütze sitzen auf dem Gitter und ziehen mit ihm vorbei.
const GRID_SPEED: f32 = 140.0;
/// Mittelpunkt eines Geschützes über bzw. unter der Mitte; der Sockel liegt auf der Gitterlinie.
const TURRET_Y: f32 = HALF_HEIGHT - 52.0;
const SEEKER_SPEED: f32 = 210.0;
/// So schnell steuert ein Sucher höchstens nach oben oder unten.
const SEEKER_CLIMB: f32 = 170.0;
const SHARD_SPEED: f32 = 250.0;
/// Kommt das Schiff einer Mine so nahe, macht sie sich scharf …
const MINE_TRIGGER: f32 = 120.0;
/// … und geht so viele Sekunden später hoch.
const MINE_FUSE: f32 = 0.7;
/// So lange zeigt ein Bit seine 0 oder 1, bevor es umkippt.
const BIT_FLIP: f32 = 1.4;
const PACKET_SPEED: f32 = 300.0;
/// So lange steigt der Schriftzug nach einer ausgelöschten Welle auf.
const BANNER_TIME: f32 = 1.3;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FreightDestroyed>()
            .add_message::<WaveCleared>()
            .insert_resource(Spawner { wave: 1.5, freighter: FIRST_FREIGHTER })
            .init_resource::<Waves>()
            .add_systems(Update, clear_on_start)
            .add_systems(
                Update,
                (spawn_waves, fly, follow_cargo, shoot, ram_player, flash, destroy, rise_banners).chain().in_set(Gameplay),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    /// Fliegt in Schlangenlinien und schießt gezielt.
    Wobbler,
    /// Schnell und geradeaus, im Verband oder im Sturzflug.
    Dart,
    /// Kreist mit anderen um einen gemeinsamen Mittelpunkt.
    Spinner,
    /// Trägt Fracht; wer sie abschießt, bekommt eine Aufgabe.
    Freighter,
    /// Sitzt am Rand auf dem Gitter und feuert Dreiergarben.
    Turret,
    /// Zerfällt beim Abschuss in zwei Splitter.
    Splitter,
    Shard,
    /// Steuert auf die Höhe des Schiffs, bis es vorbei ist.
    Seeker,
    /// Treibt langsam; kommt das Schiff zu nahe, geht sie mit einem Schussring hoch.
    Mine,
    /// Groß und zäh, feuert gezielte Fächer.
    Cruiser,
    /// Zeigt abwechselnd 0 und 1; nur als 1 verwundbar.
    Bit,
    /// Schnell, in Strömen auf einer gemeinsamen Wellenbahn.
    Packet,
}

/// Wie eine Welle aufgebaut ist.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Formation {
    /// Schlange aus Wacklern
    Snake,
    /// Pfeile in V-Formation
    Arrow,
    /// Wackler oben und unten gleichzeitig
    Pincer,
    /// Pfeile im Sturzflug
    Dive,
    /// Kreisel im Ring
    Ring,
    /// Geschützreihe am Rand
    Turrets,
    /// Teiler im Verband
    Splitters,
    /// Sucherschwarm
    Seekers,
    /// Verstreute Minen
    MineField,
    /// Zwei Kreuzer
    Cruisers,
    /// Bits in Viererreihen
    Nibbles,
    /// Ein Strom aus Datenpaketen
    Stream,
}

impl EnemyKind {
    fn health(self) -> f32 {
        match self {
            Self::Wobbler => 3.0,
            Self::Dart => 2.0,
            Self::Spinner => 2.0,
            Self::Freighter => 6.0,
            Self::Turret => 5.0,
            Self::Splitter => 5.0,
            Self::Shard => 1.0,
            Self::Seeker => 2.0,
            Self::Mine => 2.0,
            Self::Cruiser => 10.0,
            Self::Bit => 2.0,
            Self::Packet => 1.0,
        }
    }

    fn points(self) -> u32 {
        match self {
            Self::Wobbler => 100,
            Self::Dart => 150,
            Self::Spinner => 120,
            Self::Freighter => 300,
            Self::Turret => 200,
            Self::Splitter => 150,
            Self::Shard => 50,
            Self::Seeker => 130,
            Self::Mine => 120,
            Self::Cruiser => 400,
            Self::Bit => 120,
            Self::Packet => 60,
        }
    }

    /// Credits für den Abschuss.
    fn credits(self) -> u32 {
        match self {
            Self::Shard | Self::Packet => 1,
            Self::Wobbler | Self::Spinner | Self::Seeker | Self::Mine | Self::Bit => 2,
            Self::Dart | Self::Splitter => 3,
            Self::Turret => 4,
            Self::Freighter | Self::Cruiser => 6,
        }
    }

    fn radius(self) -> f32 {
        match self {
            Self::Wobbler => 17.0,
            Self::Dart => 16.0,
            Self::Spinner => 15.0,
            Self::Freighter => 24.0,
            Self::Turret => 20.0,
            Self::Splitter => 20.0,
            Self::Shard => 10.0,
            Self::Seeker => 14.0,
            Self::Mine => 15.0,
            Self::Cruiser => 28.0,
            Self::Bit => 17.0,
            Self::Packet => 10.0,
        }
    }

    fn shape(self) -> Shape {
        match self {
            Self::Wobbler => Shape::new(MAGENTA)
                .closed(&[
                    Vec2::new(-18.0, 0.0),
                    Vec2::new(-4.0, 12.0),
                    Vec2::new(12.0, 10.0),
                    Vec2::new(18.0, 0.0),
                    Vec2::new(12.0, -10.0),
                    Vec2::new(-4.0, -12.0),
                ])
                .open(&[Vec2::new(-4.0, 12.0), Vec2::new(-14.0, 19.0)])
                .open(&[Vec2::new(-4.0, -12.0), Vec2::new(-14.0, -19.0)])
                .closed(&polygon(4, 4.0, 0.0)),
            Self::Dart => Shape::new(RED)
                .closed(&[Vec2::new(-22.0, 0.0), Vec2::new(14.0, 11.0), Vec2::new(6.0, 0.0), Vec2::new(14.0, -11.0)]),
            Self::Spinner => Shape::new(VIOLET)
                .closed(&polygon(4, 16.0, 0.0))
                .closed(&polygon(4, 7.0, std::f32::consts::FRAC_PI_4)),
            // Doppelter Rahmen; in der Mitte sitzt das Zeichen der Ladung.
            Self::Freighter => Shape::new(GOLD).closed(&polygon(6, 26.0, 0.0)).closed(&polygon(6, 19.0, 0.0)),
            // Für ein Geschütz am Boden; an der Decke wird es gespiegelt.
            Self::Turret => Shape::new(ORANGE)
                .open(&[Vec2::new(-28.0, -12.0), Vec2::new(28.0, -12.0)])
                .closed(&[
                    Vec2::new(-18.0, -12.0),
                    Vec2::new(-12.0, 2.0),
                    Vec2::new(0.0, 8.0),
                    Vec2::new(12.0, 2.0),
                    Vec2::new(18.0, -12.0),
                ])
                .open(&[Vec2::new(-3.0, 6.0), Vec2::new(-14.0, 20.0)])
                .open(&[Vec2::new(2.0, 7.0), Vec2::new(-9.0, 22.0)]),
            Self::Splitter => Shape::new(LIME)
                .closed(&polygon(4, 22.0, std::f32::consts::FRAC_PI_4))
                .closed(&polygon(4, 11.0, 0.0))
                .open(&[Vec2::new(0.0, -15.0), Vec2::new(0.0, 15.0)]),
            Self::Shard => Shape::new(LIME).closed(&polygon(4, 10.0, std::f32::consts::FRAC_PI_4)),
            Self::Seeker => Shape::new(PINK)
                .closed(&[Vec2::new(-17.0, 0.0), Vec2::new(7.0, 10.0), Vec2::new(2.0, 0.0), Vec2::new(7.0, -10.0)])
                .closed(&polygon(3, 3.0, std::f32::consts::PI)),
            Self::Mine => {
                let mut shape = Shape::new(ORANGE).closed(&polygon(8, 12.0, 0.0)).closed(&polygon(4, 3.0, 0.0));
                for i in 0..4 {
                    let dir = Vec2::from_angle(std::f32::consts::FRAC_PI_4 + i as f32 * std::f32::consts::FRAC_PI_2);
                    shape = shape.open(&[dir * 12.0, dir * 20.0]);
                }
                shape
            }
            Self::Cruiser => Shape::new(RED)
                .closed(&[
                    Vec2::new(-36.0, 0.0),
                    Vec2::new(-20.0, 12.0),
                    Vec2::new(18.0, 15.0),
                    Vec2::new(34.0, 6.0),
                    Vec2::new(34.0, -6.0),
                    Vec2::new(18.0, -15.0),
                    Vec2::new(-20.0, -12.0),
                ])
                .closed(&[Vec2::new(2.0, 6.0), Vec2::new(16.0, 6.0), Vec2::new(16.0, -6.0), Vec2::new(2.0, -6.0)])
                .open(&[Vec2::new(-20.0, 12.0), Vec2::new(-30.0, 20.0)])
                .open(&[Vec2::new(-20.0, -12.0), Vec2::new(-30.0, -20.0)]),
            Self::Bit => bit_shape(false),
            Self::Packet => Shape::new(MAGENTA)
                .closed(&polygon(4, 10.0, 0.0))
                .open(&[Vec2::new(-4.0, 0.0), Vec2::new(4.0, 0.0)]),
        }
    }
}

/// Ein Bit als Ziffer in einem Rahmen: als 1 hell und verwundbar, als 0 dunkel und gepanzert.
fn bit_shape(zero: bool) -> Shape {
    let frame = [Vec2::new(-13.0, -20.0), Vec2::new(13.0, -20.0), Vec2::new(13.0, 20.0), Vec2::new(-13.0, 20.0)];
    if zero {
        let oval: Vec<Vec2> = (0..12).map(|i| Vec2::from_angle(i as f32 / 12.0 * std::f32::consts::TAU) * Vec2::new(7.0, 12.0)).collect();
        Shape::new(scaled(GREEN, 0.4)).closed(&frame).closed(&oval)
    } else {
        Shape::new(GREEN)
            .closed(&frame)
            .open(&[Vec2::new(-5.0, 8.0), Vec2::new(2.0, 13.0), Vec2::new(2.0, -13.0)])
            .open(&[Vec2::new(-5.0, -13.0), Vec2::new(8.0, -13.0)])
    }
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub health: f32,
    /// Weißes Aufblitzen nach einem Treffer, klingt von 1 auf 0 ab.
    pub flash: f32,
    color: LinearRgba,
    age: f32,
    /// Versatz der Schlangenlinie, damit eine Kette nicht im Gleichschritt wackelt.
    wave_phase: f32,
    fire_timer: f32,
    /// Flugrichtung der Pfeile.
    heading: Vec2,
    /// Mittelpunkt, um den Kreisel kreisen.
    center: Vec2,
    /// Zu welcher Welle der Gegner gehört; Frachtdrohnen gehören zu keiner.
    wave: Option<u32>,
    /// Was eine Frachtdrohne geladen hat.
    cargo: Option<Reward>,
    /// Eine scharfe Mine: Sekunden bis zur Explosion.
    fuse: Option<f32>,
    /// Die Mine ist von selbst hochgegangen; das zählt nicht als Abschuss.
    detonated: bool,
    /// Ein Bit, das gerade eine 0 zeigt: Schüsse prallen ab.
    pub shielded: bool,
}

impl Enemy {
    fn new(kind: EnemyKind, rng: &mut Rng) -> Self {
        Self {
            kind,
            health: kind.health(),
            flash: 0.0,
            color: kind.shape().color,
            age: 0.0,
            wave_phase: 0.0,
            fire_timer: rng.range(1.0, 3.5),
            heading: Vec2::NEG_X,
            center: Vec2::ZERO,
            wave: None,
            cargo: None,
            fuse: None,
            detonated: false,
            shielded: false,
        }
    }
}

#[derive(Component)]
pub struct Hitbox(pub f32);

/// Eine Frachtdrohne wurde zerstört; die Aufgabe dazu startet, sobald gerade keine läuft.
#[derive(Message)]
pub struct FreightDestroyed(pub Reward);

/// Eine Welle wurde vollständig abgeschossen, ohne dass ein Gegner entkommen ist.
#[derive(Message)]
pub struct WaveCleared {
    pub size: u32,
}

/// Die Wellen, von denen noch Gegner unterwegs sind.
#[derive(Resource, Default)]
struct Waves {
    next: u32,
    open: Vec<WaveTrack>,
}

struct WaveTrack {
    id: u32,
    size: u32,
    remaining: u32,
    escaped: bool,
}

impl Waves {
    fn start(&mut self, size: u32) -> u32 {
        self.next += 1;
        self.open.push(WaveTrack { id: self.next, size, remaining: size, escaped: false });
        self.next
    }

    /// Ein Gegner der Welle ist weg. Gibt die Größe zurück, wenn damit die ganze Welle
    /// abgeschossen ist.
    fn gone(&mut self, id: u32, escaped: bool) -> Option<u32> {
        let index = self.open.iter().position(|w| w.id == id)?;
        let wave = &mut self.open[index];
        wave.remaining -= 1;
        wave.escaped |= escaped;
        if wave.remaining > 0 {
            return None;
        }
        let wave = self.open.swap_remove(index);
        (!wave.escaped).then_some(wave.size)
    }

    /// Die Welle bekommt Zuwachs, etwa Splitter eines Teilers.
    fn grow(&mut self, id: u32, count: u32) {
        if let Some(wave) = self.open.iter_mut().find(|w| w.id == id) {
            wave.size += count;
            wave.remaining += count;
        }
    }
}

/// Zeichen und Schild einer Frachtdrohne; folgt ihr, dreht sich aber nicht mit.
#[derive(Component)]
struct CargoTag(Entity);

/// Schriftzug über einer ausgelöschten Welle, steigt auf und verblasst.
#[derive(Component)]
struct RisingBanner(f32);

#[derive(Resource)]
struct Spawner {
    wave: f32,
    freighter: f32,
}

fn spawn_enemy(commands: &mut Commands, enemy: Enemy, pos: Vec2) -> Entity {
    let kind = enemy.kind;
    commands.spawn((enemy, Hitbox(kind.radius()), Targetable, kind.shape(), Transform::from_translation(pos.extend(0.0)))).id()
}

fn spawn_cargo_tag(commands: &mut Commands, freighter: Entity, reward: Reward, pos: Vec2) {
    commands
        .spawn((CargoTag(freighter), reward.icon(), Transform::from_translation(pos.extend(0.1))))
        .with_child((
            Text2d::new(reward.label()),
            TextFont {
                font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
                ..TextFont::from_font_size(15.0)
            },
            TextColor(Color::LinearRgba(scaled(reward.color(), 0.8))),
            Transform::from_xyz(0.0, -42.0, 2.0),
        ));
}

/// Das Ladungsschild folgt seiner Frachtdrohne und verschwindet mit ihr.
fn follow_cargo(
    mut commands: Commands,
    mut tags: Query<(Entity, &CargoTag, &mut Transform)>,
    freighters: Query<&Transform, (With<Enemy>, Without<CargoTag>)>,
) {
    for (entity, tag, mut transform) in &mut tags {
        match freighters.get(tag.0) {
            Ok(freighter) => transform.translation = freighter.translation.truncate().extend(0.1),
            Err(_) => commands.entity(entity).despawn(),
        }
    }
}

/// Neues Level oder Neustart am Checkpoint: Was noch fliegt, verschwindet, und Wellen wie
/// Frachtdrohnen beginnen von vorn.
fn clear_on_start(
    mut commands: Commands,
    mut started: MessageReader<LevelStarted>,
    mut rewind: MessageReader<Rewind>,
    mut waves: ResMut<Waves>,
    mut spawner: ResMut<Spawner>,
    enemies: Query<Entity, With<Enemy>>,
) {
    if started.read().count() + rewind.read().count() == 0 {
        return;
    }
    for enemy in &enemies {
        commands.entity(enemy).despawn();
    }
    *waves = Waves::default();
    *spawner = Spawner { wave: 1.5, freighter: FIRST_FREIGHTER };
}

fn spawn_waves(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    mut waves: ResMut<Waves>,
    progress: Res<LevelProgress>,
    field: Res<Playfield>,
    current: Res<CurrentLevel>,
    campaign: Res<Campaign>,
    loadout: Res<Loadout>,
    ship: Res<Ship>,
    difficulty: Res<Difficulty>,
    player: Single<&Player>,
) {
    let dt = time.delta_secs();
    let spawn_x = field.half_width + SPAWN_MARGIN;
    spawner.wave -= dt;
    spawner.freighter -= dt;

    if spawner.freighter <= 0.0 {
        spawner.freighter = rng.range(9.0, 12.0);
        let y = rng.range(-180.0, 180.0);
        let mut freighter = Enemy::new(EnemyKind::Freighter, &mut rng);
        let cargo = Reward::choose(&mut rng, &loadout, &player, ship.hull.platforms(), campaign.beaten);
        freighter.cargo = Some(cargo);
        let pos = Vec2::new(spawn_x, y);
        let entity = spawn_enemy(&mut commands, freighter, pos);
        spawn_cargo_tag(&mut commands, entity, cargo, pos);
    }

    // Beim Boss kommen nur noch Frachtdrohnen.
    if spawner.wave > 0.0 || progress.boss_ahead() {
        return;
    }
    spawner.wave = rng.range(2.0, 3.0) * difficulty.wave_gap_factor();

    let mut members: Vec<(Enemy, Vec2)> = Vec::new();
    let formations = current.def().formations;
    match formations[rng.below(formations.len())] {
        Formation::Snake => {
            let y = rng.range(-200.0, 200.0);
            for i in 0..6 {
                let mut enemy = Enemy::new(EnemyKind::Wobbler, &mut rng);
                enemy.wave_phase = i as f32 * 0.5;
                members.push((enemy, Vec2::new(spawn_x + i as f32 * 55.0, y)));
            }
        }
        Formation::Arrow => {
            let y = rng.range(-170.0, 170.0);
            for (dx, dy) in [(0.0, 0.0), (40.0, 45.0), (40.0, -45.0), (80.0, 90.0), (80.0, -90.0)] {
                members.push((Enemy::new(EnemyKind::Dart, &mut rng), Vec2::new(spawn_x + dx, y + dy)));
            }
        }
        Formation::Pincer => {
            for y in [-220.0, 220.0] {
                for i in 0..4 {
                    let mut enemy = Enemy::new(EnemyKind::Wobbler, &mut rng);
                    enemy.wave_phase = i as f32 * 0.5;
                    members.push((enemy, Vec2::new(spawn_x + i as f32 * 55.0, y)));
                }
            }
        }
        // Die Pfeile stoßen schräg von oben oder unten herab.
        Formation::Dive => {
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let heading = Vec2::new(-0.75, -0.66 * side).normalize();
            for i in 0..5 {
                let mut enemy = Enemy::new(EnemyKind::Dart, &mut rng);
                enemy.heading = heading;
                let pos = Vec2::new(field.half_width - 490.0 + i as f32 * 85.0, side * (HALF_HEIGHT + 40.0 + i as f32 * 45.0));
                members.push((enemy, pos));
            }
        }
        // Ein Ring, der sich beim Heranfliegen dreht
        Formation::Ring => {
            let center = Vec2::new(spawn_x + 90.0, rng.range(-170.0, 170.0));
            for i in 0..6 {
                let mut enemy = Enemy::new(EnemyKind::Spinner, &mut rng);
                enemy.center = center;
                let offset = Vec2::from_angle(i as f32 / 6.0 * std::f32::consts::TAU) * 75.0;
                members.push((enemy, center + offset));
            }
        }
        // Drei Geschütze an Boden oder Decke, manchmal zwei weitere gegenüber
        Formation::Turrets => {
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            for i in 0..3 {
                members.push((Enemy::new(EnemyKind::Turret, &mut rng), Vec2::new(spawn_x + i as f32 * 220.0, side * TURRET_Y)));
            }
            if rng.chance(0.3) {
                for i in 0..2 {
                    let x = spawn_x + 110.0 + i as f32 * 220.0;
                    members.push((Enemy::new(EnemyKind::Turret, &mut rng), Vec2::new(x, -side * TURRET_Y)));
                }
            }
        }
        // Drei Teiler, leicht versetzt übereinander
        Formation::Splitters => {
            let y = rng.range(-140.0, 140.0);
            for i in 0..3 {
                let mut enemy = Enemy::new(EnemyKind::Splitter, &mut rng);
                enemy.wave_phase = i as f32 * 0.8;
                members.push((enemy, Vec2::new(spawn_x + i as f32 * 70.0, y + (i as f32 - 1.0) * 110.0)));
            }
        }
        // Fünf Minen, verstreut
        Formation::MineField => {
            for i in 0..5 {
                let pos = Vec2::new(spawn_x + i as f32 * 110.0 + rng.range(0.0, 40.0), rng.range(-250.0, 250.0));
                let mut mine = Enemy::new(EnemyKind::Mine, &mut rng);
                mine.wave_phase = rng.range(0.0, 6.0);
                members.push((mine, pos));
            }
        }
        // Zwei Kreuzer, versetzt
        Formation::Cruisers => {
            let y = rng.range(80.0, 200.0);
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            for (i, dy) in [y * side, -y * side * 0.6].into_iter().enumerate() {
                let mut cruiser = Enemy::new(EnemyKind::Cruiser, &mut rng);
                cruiser.fire_timer = 1.5 + i as f32 * 1.6;
                cruiser.wave_phase = i as f32 * 2.0;
                members.push((cruiser, Vec2::new(spawn_x + 40.0 + i as f32 * 160.0, dy)));
            }
        }
        // Eine Viererreihe aus Bits, manchmal zwei; jedes kippt zu seiner eigenen Zeit um.
        Formation::Nibbles => {
            let columns = if rng.chance(0.5) { 2 } else { 1 };
            for c in 0..columns {
                let y = rng.range(-150.0, 150.0);
                for i in 0..4 {
                    let mut bit = Enemy::new(EnemyKind::Bit, &mut rng);
                    bit.fire_timer = rng.range(0.3, BIT_FLIP);
                    bit.wave_phase = c as f32 * 1.5;
                    members.push((bit, Vec2::new(spawn_x + c as f32 * 170.0, y + (i as f32 - 1.5) * 50.0)));
                }
            }
        }
        // Acht Pakete auf derselben Wellenbahn
        Formation::Stream => {
            let base = Vec2::new(rng.range(0.0, 6.0), rng.range(-120.0, 120.0));
            for i in 0..8 {
                let mut packet = Enemy::new(EnemyKind::Packet, &mut rng);
                packet.center = base;
                members.push((packet, Vec2::new(spawn_x + i as f32 * 45.0, base.y)));
            }
        }
        // Sechs Sucher, verstreut über die ganze Höhe
        Formation::Seekers => {
            for i in 0..6 {
                let y = rng.range(-250.0, 250.0);
                let mut enemy = Enemy::new(EnemyKind::Seeker, &mut rng);
                // Jeder zielt etwas über oder unter das Schiff, damit der Schwarm nicht zur Linie wird.
                enemy.wave_phase = rng.range(-80.0, 80.0);
                members.push((enemy, Vec2::new(spawn_x + i as f32 * 80.0, y)));
            }
        }
    }
    let id = waves.start(members.len() as u32);
    for (mut enemy, pos) in members {
        enemy.wave = Some(id);
        let flipped = enemy.kind == EnemyKind::Turret && pos.y > 0.0;
        let entity = spawn_enemy(&mut commands, enemy, pos);
        if flipped {
            // An der Decke hängt das Geschütz kopfüber.
            commands.entity(entity).insert(Transform::from_translation(pos.extend(0.0)).with_scale(Vec3::new(1.0, -1.0, 1.0)));
        }
    }
}

fn fly(
    mut commands: Commands,
    time: Res<Time>,
    field: Res<Playfield>,
    mut waves: ResMut<Waves>,
    phase: Res<State<GamePhase>>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Transform, &mut Shape)>,
    player: Single<(&Transform, &Player), Without<Enemy>>,
) {
    let dt = time.delta_secs();
    let (player_transform, player) = player.into_inner();
    let target = player_transform.translation.truncate();
    let protected = player.protected(*phase.get());
    for (entity, mut enemy, mut transform, mut shape) in &mut enemies {
        enemy.age += dt;
        let t = enemy.age * 2.5 + enemy.wave_phase;
        // Geschwindigkeiten statt fester Bahnen, damit die Schutzblase Gegner wegschieben kann.
        let vel = match enemy.kind {
            EnemyKind::Wobbler => Vec2::new(-150.0, t.cos() * 70.0 * 2.5),
            EnemyKind::Dart => enemy.heading * DART_SPEED,
            EnemyKind::Spinner => {
                enemy.center.x -= RING_SPEED * dt;
                let offset = transform.translation.truncate() - enemy.center;
                Vec2::new(-RING_SPEED, 0.0) + offset.perp() * RING_SPIN
            }
            EnemyKind::Freighter => Vec2::new(-110.0, (enemy.age * 1.5).cos() * 30.0),
            EnemyKind::Turret => Vec2::new(-GRID_SPEED, 0.0),
            EnemyKind::Splitter => Vec2::new(-115.0, (enemy.age * 1.6 + enemy.wave_phase).sin() * 45.0),
            EnemyKind::Shard => enemy.heading * SHARD_SPEED,
            EnemyKind::Seeker => {
                // Nachsteuern, solange das Schiff noch vor ihm liegt; danach geradeaus weiter.
                let pos = transform.translation.truncate();
                if pos.x > target.x + 120.0 {
                    let aim = target.y + enemy.wave_phase;
                    enemy.heading.y = ((aim - pos.y) * 2.2).clamp(-SEEKER_CLIMB, SEEKER_CLIMB);
                }
                Vec2::new(-SEEKER_SPEED, enemy.heading.y)
            }
            EnemyKind::Mine => Vec2::new(-90.0, (enemy.age * 1.3 + enemy.wave_phase).sin() * 25.0),
            EnemyKind::Cruiser => Vec2::new(-105.0, (enemy.age * 0.8 + enemy.wave_phase).sin() * 25.0),
            EnemyKind::Bit => Vec2::new(-130.0, (enemy.age * 1.5 + enemy.wave_phase).cos() * 30.0),
            EnemyKind::Packet => Vec2::new(-PACKET_SPEED, 0.0),
        };
        transform.translation += (vel * dt).extend(0.0);
        match enemy.kind {
            // Alle Pakete eines Stroms folgen derselben Wellenbahn, die Höhe hängt nur von x ab.
            EnemyKind::Packet => {
                transform.translation.y = enemy.center.y + (transform.translation.x * 0.011 + enemy.center.x).sin() * 150.0;
            }
            EnemyKind::Bit => {
                enemy.fire_timer -= dt;
                if enemy.fire_timer <= 0.0 {
                    enemy.fire_timer = BIT_FLIP;
                    enemy.shielded = !enemy.shielded;
                    *shape = bit_shape(enemy.shielded);
                    enemy.color = shape.color;
                }
            }
            EnemyKind::Mine => {
                let near = transform.translation.truncate().distance(target) < MINE_TRIGGER;
                if enemy.fuse.is_none() && near && !protected {
                    enemy.fuse = Some(MINE_FUSE);
                }
                if let Some(fuse) = enemy.fuse {
                    let fuse = fuse - dt;
                    enemy.fuse = Some(fuse);
                    // Hektisches Blinken, solange sie scharf ist.
                    if (fuse * 12.0) as i32 % 2 == 0 {
                        enemy.flash = 1.0;
                    }
                    if fuse <= 0.0 && enemy.health > 0.0 {
                        enemy.detonated = true;
                        enemy.health = 0.0;
                    }
                }
            }
            _ => {}
        }
        match enemy.kind {
            EnemyKind::Freighter => transform.rotate_z(0.8 * dt),
            EnemyKind::Spinner => transform.rotate_z(3.0 * dt),
            EnemyKind::Splitter => transform.rotate_z(0.7 * dt),
            EnemyKind::Shard => transform.rotate_z(5.0 * dt),
            EnemyKind::Seeker => transform.rotation = Quat::from_rotation_z(vel.to_angle() - std::f32::consts::PI),
            _ => {}
        }
        // Entkommen: links hinaus oder nach einem Sturzflug oben bzw. unten.
        let p = transform.translation;
        if p.x < -field.half_width - 120.0 || (enemy.age > 1.0 && p.y.abs() > HALF_HEIGHT + 160.0) {
            if let Some(wave) = enemy.wave {
                waves.gone(wave, true);
            }
            commands.entity(entity).despawn();
        }
    }
}

fn shoot(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    field: Res<Playfield>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
    player: Single<&Transform, With<Player>>,
) {
    let target = player.translation.truncate();
    for (mut enemy, transform) in &mut enemies {
        if !matches!(enemy.kind, EnemyKind::Wobbler | EnemyKind::Turret | EnemyKind::Cruiser) {
            continue;
        }
        enemy.fire_timer -= time.delta_secs();
        let pos = transform.translation.truncate();
        if enemy.fire_timer > 0.0 || pos.x > field.half_width - 20.0 {
            continue;
        }
        if enemy.kind == EnemyKind::Wobbler {
            if pos.x < target.x {
                continue;
            }
            enemy.fire_timer = rng.range(2.0, 4.5);
            let dir = (target - pos).normalize_or(Vec2::NEG_X);
            spawn_enemy_shot(&mut commands, pos, dir * ENEMY_SHOT_SPEED);
        } else if enemy.kind == EnemyKind::Cruiser {
            // Kreuzer: ein Fächer aus fünf Schüssen in Richtung Schiff.
            if pos.x < target.x + 100.0 {
                continue;
            }
            enemy.fire_timer = rng.range(3.8, 5.0);
            let muzzle = pos + Vec2::new(-34.0, 0.0);
            let aim = (target - muzzle).normalize_or(Vec2::NEG_X).to_angle();
            for i in 0..5 {
                let spread = (i as f32 - 2.0) * 0.16;
                spawn_enemy_shot(&mut commands, muzzle, Vec2::from_angle(aim + spread) * ENEMY_SHOT_SPEED);
            }
        } else {
            // Geschütz: eine Garbe aus drei Schüssen in Richtung Schiff, aus dem Rohr heraus.
            if pos.x < -field.half_width + 40.0 {
                continue;
            }
            enemy.fire_timer = rng.range(3.6, 5.0);
            let muzzle = pos + Vec2::new(-12.0, 16.0 * -pos.y.signum());
            let aim = (target - muzzle).normalize_or(Vec2::NEG_X).to_angle();
            for spread in [-0.2, 0.0, 0.2] {
                spawn_enemy_shot(&mut commands, muzzle, Vec2::from_angle(aim + spread) * ENEMY_SHOT_SPEED);
            }
        }
    }
}

/// Zusammenstöße mit dem Schiff. Ist es geschützt, schiebt die Blase Gegner zur Seite,
/// statt sie explodieren zu lassen: Wer rechnet, wird nicht gerammt.
fn ram_player(
    phase: Res<State<GamePhase>>,
    mut enemies: Query<(&mut Enemy, &mut Transform, &Hitbox)>,
    player: Single<(&mut Player, &Transform), Without<Enemy>>,
) {
    let (mut player, player_transform) = player.into_inner();
    let center = player_transform.translation.truncate();
    let protected = player.protected(*phase.get());
    for (mut enemy, mut transform, hitbox) in &mut enemies {
        let pos = transform.translation.truncate();
        let offset = pos - center;
        if protected {
            let min = BUBBLE_RADIUS + hitbox.0;
            if offset.length() < min {
                let away = offset.normalize_or(Vec2::Y);
                transform.translation = (center + away * min).extend(transform.translation.z);
            }
        } else if offset.length() < HIT_RADIUS + hitbox.0 && enemy.health > 0.0 {
            player.hurt(RAM_DAMAGE);
            enemy.health = 0.0;
        }
    }
}

fn flash(time: Res<Time>, mut enemies: Query<(&mut Enemy, &mut Shape)>) {
    for (mut enemy, mut shape) in &mut enemies {
        enemy.flash = (enemy.flash - time.delta_secs() * 8.0).max(0.0);
        shape.color = enemy.color.mix(&WHITE, enemy.flash);
    }
}

fn destroy(
    mut commands: Commands,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut score: ResMut<Score>,
    mut freight: MessageWriter<FreightDestroyed>,
    mut cleared: MessageWriter<WaveCleared>,
    mut waves: ResMut<Waves>,
    mut shake: ResMut<Shake>,
    mut credits: ResMut<Credits>,
    field: Res<Playfield>,
    enemies: Query<(Entity, &Enemy, &Shape, &Transform)>,
) {
    for (entity, enemy, shape, transform) in &enemies {
        if enemy.health > 0.0 {
            continue;
        }
        let mut shape = shape.clone();
        shape.color = enemy.color;
        let size = if matches!(enemy.kind, EnemyKind::Freighter | EnemyKind::Cruiser) { 1.6 } else { 1.0 };
        particles.explode(&mut rng, &shape, transform, size);
        play(&mut commands, Sfx::Explosion);
        // Eine Mine, die von selbst hochgeht: Schussring, keine Punkte, die Welle gilt als entkommen.
        if enemy.detonated {
            info!("Mine gezündet");
            let pos = transform.translation.truncate();
            particles.ring(pos, ORANGE, 160.0, 0.3);
            for i in 0..8 {
                let dir = Vec2::from_angle(i as f32 / 8.0 * std::f32::consts::TAU);
                spawn_enemy_shot(&mut commands, pos + dir * 14.0, dir * ENEMY_SHOT_SPEED * 0.85);
            }
            if let Some(wave) = enemy.wave {
                waves.gone(wave, true);
            }
            commands.entity(entity).despawn();
            continue;
        }
        score.0 += enemy.kind.points();
        credits.flight += enemy.kind.credits();
        if enemy.kind == EnemyKind::Freighter {
            freight.write(FreightDestroyed(enemy.cargo.unwrap_or(Reward::Repair)));
        }
        // Ein Teiler zerfällt in zwei Splitter, die zur selben Welle gehören.
        if enemy.kind == EnemyKind::Splitter {
            if let Some(wave) = enemy.wave {
                waves.grow(wave, 2);
            }
            for side in [1.0, -1.0] {
                let mut shard = Enemy::new(EnemyKind::Shard, &mut rng);
                shard.heading = Vec2::new(-0.75, 0.66 * side).normalize();
                shard.wave = enemy.wave;
                spawn_enemy(&mut commands, shard, transform.translation.truncate() + Vec2::Y * 12.0 * side);
            }
        }
        if let Some(size) = enemy.wave.and_then(|wave| waves.gone(wave, false)) {
            cleared.write(WaveCleared { size });
            wave_cleared_effect(&mut commands, &mut particles, &mut rng, &mut shake, &field, transform.translation.truncate());
        }
        commands.entity(entity).despawn();
    }
}

/// Goldene Doppel-Druckwelle, Funkenregen und ein Schriftzug, wo der letzte Gegner der Welle fiel.
fn wave_cleared_effect(
    commands: &mut Commands,
    particles: &mut Particles,
    rng: &mut Rng,
    shake: &mut Shake,
    field: &Playfield,
    at: Vec2,
) {
    particles.ring(at, GOLD, 520.0, 0.55);
    particles.ring(at, scaled(WHITE, 0.8), 340.0, 0.45);
    particles.sparks(rng, at, GOLD, 60, 520.0);
    particles.sparks(rng, at, WHITE, 20, 300.0);
    shake.0 = shake.0.max(5.0);
    // Über der Druckwelle und nicht zu nah am Rand, damit der Schriftzug lesbar bleibt.
    let pos = (at + Vec2::Y * 80.0).clamp(Vec2::new(-field.half_width + 160.0, -HALF_HEIGHT + 80.0), Vec2::new(field.half_width - 160.0, HALF_HEIGHT - 120.0));
    commands.spawn((
        RisingBanner(0.0),
        Text2d::new("Welle ausgelöscht!"),
        TextFont {
            font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
            ..TextFont::from_font_size(30.0)
        },
        TextColor(Color::LinearRgba(GOLD)),
        Transform::from_translation(pos.extend(2.0)),
    ));
}

fn rise_banners(mut commands: Commands, time: Res<Time>, mut banners: Query<(Entity, &mut RisingBanner, &mut Transform, &mut TextColor)>) {
    let dt = time.delta_secs();
    for (entity, mut banner, mut transform, mut color) in &mut banners {
        banner.0 += dt;
        let t = banner.0 / BANNER_TIME;
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation.y += 50.0 * dt;
        // Kurz aufploppen, dann verblassen.
        transform.scale = Vec3::splat(1.0 + 0.4 * (1.0 - (t * 6.0).min(1.0)));
        color.0 = Color::LinearRgba(scaled(GOLD, 1.0 - t * t));
    }
}
