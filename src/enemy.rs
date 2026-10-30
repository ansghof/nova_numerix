//! Gegnerwellen, die von rechts hereinfliegen, und die goldenen Frachtdrohnen, deren Abschuss
//! eine Aufgabe auslöst.

use bevy::prelude::*;

use crate::hud::Score;
use crate::neon::{GOLD, MAGENTA, RED, Shape, VIOLET, WHITE, polygon, scaled};
use crate::particles::Particles;
use crate::player::{BUBBLE_RADIUS, HIT_RADIUS, Player};
use crate::rng::Rng;
use crate::level::LevelProgress;
use crate::shots::{Targetable, spawn_enemy_shot};
use crate::{GamePhase, Gameplay, HALF_HEIGHT, HALF_WIDTH, Shake};

const SPAWN_X: f32 = HALF_WIDTH + 60.0;
const RAM_DAMAGE: f32 = 25.0;
const ENEMY_SHOT_SPEED: f32 = 260.0;
/// Die erste Frachtdrohne kommt früh, damit man die Aufgaben gleich kennenlernt.
const FIRST_FREIGHTER: f32 = 6.0;
const DART_SPEED: f32 = 380.0;
/// Wie schnell ein Kreisel-Verband nach links zieht und sich dabei dreht.
const RING_SPEED: f32 = 120.0;
const RING_SPIN: f32 = 1.8;
/// So lange steigt der Schriftzug nach einer ausgelöschten Welle auf.
const BANNER_TIME: f32 = 1.3;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FreightDestroyed>()
            .add_message::<WaveCleared>()
            .insert_resource(Spawner { wave: 1.5, freighter: FIRST_FREIGHTER })
            .init_resource::<Waves>()
            .add_systems(
                Update,
                (spawn_waves, fly, shoot, ram_player, flash, destroy, rise_banners).chain().in_set(Gameplay),
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
}

impl EnemyKind {
    fn health(self) -> f32 {
        match self {
            Self::Wobbler => 3.0,
            Self::Dart => 2.0,
            Self::Spinner => 2.0,
            Self::Freighter => 6.0,
        }
    }

    fn points(self) -> u32 {
        match self {
            Self::Wobbler => 100,
            Self::Dart => 150,
            Self::Spinner => 120,
            Self::Freighter => 300,
        }
    }

    fn radius(self) -> f32 {
        match self {
            Self::Wobbler => 17.0,
            Self::Dart => 16.0,
            Self::Spinner => 15.0,
            Self::Freighter => 24.0,
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
            Self::Freighter => Shape::new(GOLD)
                .closed(&polygon(6, 24.0, 0.0))
                .closed(&polygon(4, 11.0, std::f32::consts::FRAC_PI_4))
                .open(&[Vec2::new(-11.0, 0.0), Vec2::new(11.0, 0.0)])
                .open(&[Vec2::new(0.0, -11.0), Vec2::new(0.0, 11.0)]),
        }
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
        }
    }
}

#[derive(Component)]
pub struct Hitbox(pub f32);

/// Eine Frachtdrohne wurde zerstört; die Aufgabe dazu startet, sobald gerade keine läuft.
#[derive(Message)]
pub struct FreightDestroyed;

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
}

/// Schriftzug über einer ausgelöschten Welle, steigt auf und verblasst.
#[derive(Component)]
struct RisingBanner(f32);

#[derive(Resource)]
struct Spawner {
    wave: f32,
    freighter: f32,
}

fn spawn_enemy(commands: &mut Commands, enemy: Enemy, pos: Vec2) {
    let kind = enemy.kind;
    commands.spawn((enemy, Hitbox(kind.radius()), Targetable, kind.shape(), Transform::from_translation(pos.extend(0.0))));
}

fn spawn_waves(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    mut waves: ResMut<Waves>,
    progress: Res<LevelProgress>,
) {
    let dt = time.delta_secs();
    spawner.wave -= dt;
    spawner.freighter -= dt;

    if spawner.freighter <= 0.0 {
        spawner.freighter = rng.range(9.0, 12.0);
        let y = rng.range(-180.0, 180.0);
        let freighter = Enemy::new(EnemyKind::Freighter, &mut rng);
        spawn_enemy(&mut commands, freighter, Vec2::new(SPAWN_X, y));
    }

    // Beim Boss kommen nur noch Frachtdrohnen.
    if spawner.wave > 0.0 || progress.boss_ahead() {
        return;
    }
    spawner.wave = rng.range(2.0, 3.0);

    let mut members: Vec<(Enemy, Vec2)> = Vec::new();
    match rng.below(5) {
        // Schlange aus Wackelschiffen
        0 => {
            let y = rng.range(-200.0, 200.0);
            for i in 0..6 {
                let mut enemy = Enemy::new(EnemyKind::Wobbler, &mut rng);
                enemy.wave_phase = i as f32 * 0.5;
                members.push((enemy, Vec2::new(SPAWN_X + i as f32 * 55.0, y)));
            }
        }
        // Pfeile in V-Formation
        1 => {
            let y = rng.range(-170.0, 170.0);
            for (dx, dy) in [(0.0, 0.0), (40.0, 45.0), (40.0, -45.0), (80.0, 90.0), (80.0, -90.0)] {
                members.push((Enemy::new(EnemyKind::Dart, &mut rng), Vec2::new(SPAWN_X + dx, y + dy)));
            }
        }
        // Zange: oben und unten gleichzeitig
        2 => {
            for y in [-220.0, 220.0] {
                for i in 0..4 {
                    let mut enemy = Enemy::new(EnemyKind::Wobbler, &mut rng);
                    enemy.wave_phase = i as f32 * 0.5;
                    members.push((enemy, Vec2::new(SPAWN_X + i as f32 * 55.0, y)));
                }
            }
        }
        // Sturzflug: Pfeile stoßen schräg von oben oder unten herab
        3 => {
            let side = if rng.chance(0.5) { 1.0 } else { -1.0 };
            let heading = Vec2::new(-0.75, -0.66 * side).normalize();
            for i in 0..5 {
                let mut enemy = Enemy::new(EnemyKind::Dart, &mut rng);
                enemy.heading = heading;
                let pos = Vec2::new(150.0 + i as f32 * 85.0, side * (HALF_HEIGHT + 40.0 + i as f32 * 45.0));
                members.push((enemy, pos));
            }
        }
        // Kreisel: ein Ring, der sich beim Heranfliegen dreht
        _ => {
            let center = Vec2::new(SPAWN_X + 90.0, rng.range(-170.0, 170.0));
            for i in 0..6 {
                let mut enemy = Enemy::new(EnemyKind::Spinner, &mut rng);
                enemy.center = center;
                let offset = Vec2::from_angle(i as f32 / 6.0 * std::f32::consts::TAU) * 75.0;
                members.push((enemy, center + offset));
            }
        }
    }
    let id = waves.start(members.len() as u32);
    for (mut enemy, pos) in members {
        enemy.wave = Some(id);
        spawn_enemy(&mut commands, enemy, pos);
    }
}

fn fly(
    mut commands: Commands,
    time: Res<Time>,
    mut waves: ResMut<Waves>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut enemy, mut transform) in &mut enemies {
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
        };
        transform.translation += (vel * dt).extend(0.0);
        match enemy.kind {
            EnemyKind::Freighter => transform.rotate_z(0.8 * dt),
            EnemyKind::Spinner => transform.rotate_z(3.0 * dt),
            _ => {}
        }
        // Entkommen: links hinaus oder nach einem Sturzflug oben bzw. unten.
        let p = transform.translation;
        if p.x < -HALF_WIDTH - 120.0 || (enemy.age > 1.0 && p.y.abs() > HALF_HEIGHT + 160.0) {
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
    mut enemies: Query<(&mut Enemy, &Transform)>,
    player: Single<&Transform, With<Player>>,
) {
    let target = player.translation.truncate();
    for (mut enemy, transform) in &mut enemies {
        if enemy.kind != EnemyKind::Wobbler {
            continue;
        }
        enemy.fire_timer -= time.delta_secs();
        let pos = transform.translation.truncate();
        if enemy.fire_timer > 0.0 || pos.x > HALF_WIDTH - 20.0 || pos.x < target.x {
            continue;
        }
        enemy.fire_timer = rng.range(2.0, 4.5);
        let dir = (target - pos).normalize_or(Vec2::NEG_X);
        spawn_enemy_shot(&mut commands, pos, dir * ENEMY_SHOT_SPEED);
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
            player.health -= RAM_DAMAGE;
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
    enemies: Query<(Entity, &Enemy, &Shape, &Transform)>,
) {
    for (entity, enemy, shape, transform) in &enemies {
        if enemy.health > 0.0 {
            continue;
        }
        let mut shape = shape.clone();
        shape.color = enemy.color;
        let size = if enemy.kind == EnemyKind::Freighter { 1.6 } else { 1.0 };
        particles.explode(&mut rng, &shape, transform, size);
        score.0 += enemy.kind.points();
        if enemy.kind == EnemyKind::Freighter {
            freight.write(FreightDestroyed);
        }
        if let Some(size) = enemy.wave.and_then(|wave| waves.gone(wave, false)) {
            cleared.write(WaveCleared { size });
            wave_cleared_effect(&mut commands, &mut particles, &mut rng, &mut shake, transform.translation.truncate());
        }
        commands.entity(entity).despawn();
    }
}

/// Goldene Doppel-Druckwelle, Funkenregen und ein Schriftzug, wo der letzte Gegner der Welle fiel.
fn wave_cleared_effect(commands: &mut Commands, particles: &mut Particles, rng: &mut Rng, shake: &mut Shake, at: Vec2) {
    particles.ring(at, GOLD, 520.0, 0.55);
    particles.ring(at, scaled(WHITE, 0.8), 340.0, 0.45);
    particles.sparks(rng, at, GOLD, 60, 520.0);
    particles.sparks(rng, at, WHITE, 20, 300.0);
    shake.0 = shake.0.max(5.0);
    // Nicht zu nah an den Rand, damit der Schriftzug lesbar bleibt.
    let pos = at.clamp(Vec2::new(-HALF_WIDTH + 160.0, -HALF_HEIGHT + 80.0), Vec2::new(HALF_WIDTH - 160.0, HALF_HEIGHT - 120.0));
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
