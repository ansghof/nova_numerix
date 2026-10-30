//! Gegnerwellen, die von rechts hereinfliegen, und die goldenen Frachtdrohnen, deren Abschuss
//! eine Aufgabe auslöst.

use bevy::prelude::*;

use crate::hud::Score;
use crate::neon::{GOLD, MAGENTA, RED, Shape, WHITE, polygon};
use crate::particles::Particles;
use crate::player::{BUBBLE_RADIUS, HIT_RADIUS, Player};
use crate::rng::Rng;
use crate::level::LevelProgress;
use crate::shots::{Targetable, spawn_enemy_shot};
use crate::{GamePhase, HALF_WIDTH};

const SPAWN_X: f32 = HALF_WIDTH + 60.0;
const RAM_DAMAGE: f32 = 25.0;
const ENEMY_SHOT_SPEED: f32 = 260.0;
/// Die erste Frachtdrohne kommt früh, damit man die Aufgaben gleich kennenlernt.
const FIRST_FREIGHTER: f32 = 6.0;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FreightDestroyed>()
            .insert_resource(Spawner { wave: 2.0, freighter: FIRST_FREIGHTER })
            .add_systems(
                Update,
                (spawn_waves, fly, shoot, ram_player, flash, destroy).chain(),
            );
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnemyKind {
    /// Fliegt in Schlangenlinien und schießt gezielt.
    Wobbler,
    /// Schnell und geradeaus, im Verband.
    Dart,
    /// Trägt Fracht; wer sie abschießt, bekommt eine Aufgabe.
    Freighter,
}

impl EnemyKind {
    fn health(self) -> f32 {
        match self {
            Self::Wobbler => 3.0,
            Self::Dart => 2.0,
            Self::Freighter => 6.0,
        }
    }

    fn points(self) -> u32 {
        match self {
            Self::Wobbler => 100,
            Self::Dart => 150,
            Self::Freighter => 300,
        }
    }

    fn radius(self) -> f32 {
        match self {
            Self::Wobbler => 17.0,
            Self::Dart => 16.0,
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
}

#[derive(Component)]
pub struct Hitbox(pub f32);

/// Eine Frachtdrohne wurde zerstört; die Aufgabe dazu startet, sobald gerade keine läuft.
#[derive(Message)]
pub struct FreightDestroyed;

#[derive(Resource)]
struct Spawner {
    wave: f32,
    freighter: f32,
}

fn spawn_enemy(commands: &mut Commands, rng: &mut Rng, kind: EnemyKind, pos: Vec2, wave_phase: f32) {
    let shape = kind.shape();
    commands.spawn((
        Enemy {
            kind,
            health: kind.health(),
            flash: 0.0,
            color: shape.color,
            age: 0.0,
            wave_phase,
            fire_timer: rng.range(1.0, 3.5),
        },
        Hitbox(kind.radius()),
        Targetable,
        shape,
        Transform::from_translation(pos.extend(0.0)),
    ));
}

fn spawn_waves(
    mut commands: Commands,
    time: Res<Time>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    progress: Res<LevelProgress>,
) {
    let dt = time.delta_secs();
    spawner.wave -= dt;
    spawner.freighter -= dt;

    if spawner.freighter <= 0.0 {
        spawner.freighter = rng.range(10.0, 14.0);
        let y = rng.range(-180.0, 180.0);
        spawn_enemy(&mut commands, &mut rng, EnemyKind::Freighter, Vec2::new(SPAWN_X, y), 0.0);
    }

    // Beim Boss kommen nur noch Frachtdrohnen.
    if spawner.wave > 0.0 || progress.boss_ahead() {
        return;
    }
    spawner.wave = rng.range(2.8, 4.2);
    match rng.below(3) {
        // Schlange aus Wackelschiffen
        0 => {
            let y = rng.range(-200.0, 200.0);
            for i in 0..6 {
                let pos = Vec2::new(SPAWN_X + i as f32 * 55.0, y);
                spawn_enemy(&mut commands, &mut rng, EnemyKind::Wobbler, pos, i as f32 * 0.5);
            }
        }
        // Pfeile in V-Formation
        1 => {
            let y = rng.range(-170.0, 170.0);
            for (dx, dy) in [(0.0, 0.0), (40.0, 45.0), (40.0, -45.0), (80.0, 90.0), (80.0, -90.0)] {
                spawn_enemy(&mut commands, &mut rng, EnemyKind::Dart, Vec2::new(SPAWN_X + dx, y + dy), 0.0);
            }
        }
        // Zange: oben und unten gleichzeitig
        _ => {
            for y in [-220.0, 220.0] {
                for i in 0..4 {
                    let pos = Vec2::new(SPAWN_X + i as f32 * 55.0, y);
                    spawn_enemy(&mut commands, &mut rng, EnemyKind::Wobbler, pos, i as f32 * 0.5);
                }
            }
        }
    }
}

fn fly(mut commands: Commands, time: Res<Time>, mut enemies: Query<(Entity, &mut Enemy, &mut Transform)>) {
    let dt = time.delta_secs();
    for (entity, mut enemy, mut transform) in &mut enemies {
        enemy.age += dt;
        let t = enemy.age * 2.5 + enemy.wave_phase;
        // Geschwindigkeiten statt fester Bahnen, damit die Schutzblase Gegner wegschieben kann.
        let vel = match enemy.kind {
            EnemyKind::Wobbler => Vec2::new(-150.0, t.cos() * 70.0 * 2.5),
            EnemyKind::Dart => Vec2::new(-380.0, 0.0),
            EnemyKind::Freighter => Vec2::new(-110.0, (enemy.age * 1.5).cos() * 30.0),
        };
        transform.translation += (vel * dt).extend(0.0);
        if enemy.kind == EnemyKind::Freighter {
            transform.rotate_z(0.8 * dt);
        }
        if transform.translation.x < -HALF_WIDTH - 120.0 {
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
        commands.entity(entity).despawn();
    }
}
