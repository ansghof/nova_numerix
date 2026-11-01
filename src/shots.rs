//! Geschosse beider Seiten und was passiert, wenn sie treffen.

use bevy::prelude::*;

use crate::enemy::{Enemy, Hitbox};
use crate::neon::{CYAN, ORANGE, Shape, WHITE, polygon, scaled};
use crate::particles::Particles;
use crate::player::{BUBBLE_RADIUS, HIT_RADIUS, Player};
use crate::rng::Rng;
use crate::{GamePhase, Gameplay, HALF_HEIGHT, Playfield};

const LASER_SPEED: f32 = 1100.0;
const MISSILE_SPEED: f32 = 520.0;
const SIDE_SHOT_SPEED: f32 = 900.0;
const PLASMA_SPEED: f32 = 480.0;
/// Leuchtfarbe des Plasmas: helles Violett.
pub const PLASMA: LinearRgba = LinearRgba::rgb(2.4, 0.9, 3.8);
/// So schnell dreht eine Rakete zum Ziel, in Bogenmaß pro Sekunde.
const MISSILE_TURN: f32 = 5.0;
const ENEMY_SHOT_DAMAGE: f32 = 10.0;

pub struct ShotsPlugin;

impl Plugin for ShotsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (steer_missiles, fly, hit_enemies, hit_player).chain().in_set(Gameplay))
            .add_systems(Update, clear_on_start);
    }
}

#[derive(Component)]
pub struct Shot {
    pub vel: Vec2,
    pub damage: f32,
    pub radius: f32,
}

/// Vom Spieler abgefeuert.
#[derive(Component)]
pub struct Friendly;

/// Von Gegnern abgefeuert.
#[derive(Component)]
pub struct Hostile;

#[derive(Component)]
struct Homing;

/// Geht durch Gegner hindurch und trifft jeden nur einmal; Schilde halten es auf.
#[derive(Component)]
pub struct Piercing {
    hit: Vec<Entity>,
}

/// Raketen suchen sich ihr Ziel unter allem, was dieses Merkmal trägt.
#[derive(Component)]
pub struct Targetable;

fn oriented(pos: Vec2, dir: Vec2) -> Transform {
    Transform::from_translation(pos.extend(0.5)).with_rotation(Quat::from_rotation_z(dir.to_angle()))
}

pub fn spawn_laser(commands: &mut Commands, pos: Vec2, dir: Vec2, color: LinearRgba) {
    commands.spawn((
        Shot { vel: dir * LASER_SPEED, damage: 1.0, radius: 6.0 },
        Friendly,
        Shape::new(color).open(&[Vec2::new(-12.0, 0.0), Vec2::new(12.0, 0.0)]),
        oriented(pos, dir),
    ));
}

/// Eine Plasmawelle: ein leuchtender Bogen, mit jeder Stufe breiter und stärker.
pub fn spawn_plasma(commands: &mut Commands, pos: Vec2, level: u8) {
    let radius = 12.0 + level as f32 * 5.0;
    let arc: Vec<Vec2> = (0..=8)
        .map(|i| {
            let angle = (-70.0 + i as f32 * 17.5f32).to_radians();
            Vec2::from_angle(angle) * radius - Vec2::X * radius * 0.6
        })
        .collect();
    let inner: Vec<Vec2> = arc.iter().map(|p| *p * 0.7).collect();
    commands.spawn((
        Shot { vel: Vec2::X * PLASMA_SPEED, damage: 1.0 + level as f32, radius: radius * 0.8 },
        Friendly,
        Piercing { hit: Vec::new() },
        Shape::new(PLASMA).open(&arc).open(&inner),
        Transform::from_translation(pos.extend(0.5)),
    ));
}

/// Kurze, kräftige Bolzen der Seitenkanonen.
pub fn spawn_side_shot(commands: &mut Commands, pos: Vec2, dir: Vec2) {
    commands.spawn((
        Shot { vel: dir * SIDE_SHOT_SPEED, damage: 1.0, radius: 6.0 },
        Friendly,
        Shape::new(scaled(CYAN, 1.2)).open(&[Vec2::new(-6.0, 0.0), Vec2::new(6.0, 0.0)]).closed(&polygon(3, 3.0, 0.0)),
        oriented(pos, dir),
    ));
}

pub fn spawn_missile(commands: &mut Commands, pos: Vec2, dir: Vec2) {
    commands.spawn((
        Shot { vel: dir * MISSILE_SPEED, damage: 3.0, radius: 8.0 },
        Friendly,
        Homing,
        Shape::new(CYAN).closed(&[Vec2::new(9.0, 0.0), Vec2::new(-6.0, 4.0), Vec2::new(-6.0, -4.0)]),
        oriented(pos, dir),
    ));
}

pub fn spawn_enemy_shot(commands: &mut Commands, pos: Vec2, vel: Vec2) {
    commands.spawn((
        Shot { vel, damage: ENEMY_SHOT_DAMAGE, radius: 6.0 },
        Hostile,
        Shape::new(ORANGE).closed(&polygon(4, 7.0, 0.0)).closed(&polygon(4, 3.0, 0.0)),
        Transform::from_translation(pos.extend(0.5)),
    ));
}

fn clear_on_start(
    mut commands: Commands,
    mut started: MessageReader<crate::level::LevelStarted>,
    mut rewind: MessageReader<crate::level::Rewind>,
    shots: Query<Entity, With<Shot>>,
) {
    if started.read().count() + rewind.read().count() > 0 {
        for shot in &shots {
            commands.entity(shot).despawn();
        }
    }
}

fn steer_missiles(
    time: Res<Time>,
    mut missiles: Query<(&mut Shot, &mut Transform), With<Homing>>,
    targets: Query<&GlobalTransform, With<Targetable>>,
) {
    let dt = time.delta_secs();
    for (mut shot, mut transform) in &mut missiles {
        let pos = transform.translation.truncate();
        // Nur Ziele vor der Rakete kommen in Frage.
        let target = targets
            .iter()
            .map(|t| t.translation().truncate())
            .filter(|t| t.x > pos.x - 20.0)
            .min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)));
        if let Some(target) = target {
            let current = shot.vel.to_angle();
            let wanted = (target - pos).to_angle();
            let diff = (wanted - current + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let angle = current + diff.clamp(-MISSILE_TURN * dt, MISSILE_TURN * dt);
            shot.vel = Vec2::from_angle(angle) * MISSILE_SPEED;
        }
        transform.rotation = Quat::from_rotation_z(shot.vel.to_angle());
    }
}

fn fly(
    mut commands: Commands,
    time: Res<Time>,
    field: Res<Playfield>,
    mut shots: Query<(Entity, &Shot, &mut Transform, Has<Hostile>)>,
) {
    let dt = time.delta_secs();
    for (entity, shot, mut transform, hostile) in &mut shots {
        transform.translation += (shot.vel * dt).extend(0.0);
        // Feindliche Schüsse drehen sich, das macht sie leichter zu erkennen.
        if hostile {
            transform.rotate_z(6.0 * dt);
        }
        let p = transform.translation;
        if p.x.abs() > field.half_width + 80.0 || p.y.abs() > HALF_HEIGHT + 80.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn hit_enemies(
    mut commands: Commands,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut shots: Query<(Entity, &Shot, &Transform, Option<&mut Piercing>), With<Friendly>>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform, &Hitbox)>,
) {
    for (entity, shot, shot_transform, mut piercing) in &mut shots {
        let pos = shot_transform.translation.truncate();
        for (enemy_entity, mut enemy, transform, hitbox) in &mut enemies {
            if enemy.health <= 0.0 || piercing.as_ref().is_some_and(|p| p.hit.contains(&enemy_entity)) {
                continue;
            }
            if pos.distance(transform.translation.truncate()) < hitbox.0 + shot.radius {
                // Ein Bit, das gerade 0 zeigt, lässt den Schuss abprallen.
                if enemy.shielded {
                    particles.sparks(&mut rng, pos, scaled(CYAN, 0.5), 3, 140.0);
                    commands.entity(entity).try_despawn();
                    break;
                }
                enemy.health -= shot.damage;
                enemy.flash = 1.0;
                particles.sparks(&mut rng, pos, WHITE, 5, 220.0);
                // Plasma fliegt weiter und merkt sich, wen es schon getroffen hat.
                if let Some(piercing) = &mut piercing {
                    piercing.hit.push(enemy_entity);
                    continue;
                }
                commands.entity(entity).try_despawn();
                break;
            }
        }
    }
}

fn hit_player(
    mut commands: Commands,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    phase: Res<State<GamePhase>>,
    shots: Query<(Entity, &Shot, &Transform), With<Hostile>>,
    player: Single<(&mut Player, &Transform)>,
) {
    let (mut player, player_transform) = player.into_inner();
    let center = player_transform.translation.truncate();
    let protected = player.protected(*phase.get());
    let reach = if protected { BUBBLE_RADIUS } else { HIT_RADIUS };
    for (entity, shot, transform) in &shots {
        let pos = transform.translation.truncate();
        if pos.distance(center) > reach + shot.radius {
            continue;
        }
        if protected {
            // Prallt an der Blase ab und zerplatzt.
            particles.sparks(&mut rng, pos, CYAN, 8, 160.0);
        } else {
            player.hurt(shot.damage);
            particles.sparks(&mut rng, pos, ORANGE, 14, 260.0);
        }
        commands.entity(entity).despawn();
    }
}
