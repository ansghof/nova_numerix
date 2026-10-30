//! Geschosse beider Seiten und was passiert, wenn sie treffen.

use bevy::prelude::*;

use crate::enemy::{Enemy, Hitbox};
use crate::neon::{CYAN, ORANGE, Shape, WHITE, polygon};
use crate::particles::Particles;
use crate::player::{BUBBLE_RADIUS, HIT_RADIUS, Player};
use crate::rng::Rng;
use crate::{GamePhase, HALF_HEIGHT, HALF_WIDTH};

const LASER_SPEED: f32 = 1100.0;
const MISSILE_SPEED: f32 = 520.0;
/// So schnell dreht eine Rakete zum Ziel, in Bogenmaß pro Sekunde.
const MISSILE_TURN: f32 = 5.0;
const ENEMY_SHOT_DAMAGE: f32 = 10.0;

pub struct ShotsPlugin;

impl Plugin for ShotsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (steer_missiles, fly, hit_enemies, hit_player).chain());
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

/// Raketen suchen sich ihr Ziel unter allem, was dieses Merkmal trägt.
#[derive(Component)]
pub struct Targetable;

fn oriented(pos: Vec2, dir: Vec2) -> Transform {
    Transform::from_translation(pos.extend(0.5)).with_rotation(Quat::from_rotation_z(dir.to_angle()))
}

pub fn spawn_laser(commands: &mut Commands, pos: Vec2, dir: Vec2) {
    commands.spawn((
        Shot { vel: dir * LASER_SPEED, damage: 1.0, radius: 6.0 },
        Friendly,
        Shape::new(WHITE).open(&[Vec2::new(-12.0, 0.0), Vec2::new(12.0, 0.0)]),
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
        if p.x.abs() > HALF_WIDTH + 80.0 || p.y.abs() > HALF_HEIGHT + 80.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn hit_enemies(
    mut commands: Commands,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    shots: Query<(Entity, &Shot, &Transform), With<Friendly>>,
    mut enemies: Query<(&mut Enemy, &Transform, &Hitbox)>,
) {
    for (entity, shot, shot_transform) in &shots {
        let pos = shot_transform.translation.truncate();
        for (mut enemy, transform, hitbox) in &mut enemies {
            if enemy.health <= 0.0 {
                continue;
            }
            if pos.distance(transform.translation.truncate()) < hitbox.0 + shot.radius {
                enemy.health -= shot.damage;
                enemy.flash = 1.0;
                particles.sparks(&mut rng, pos, WHITE, 5, 220.0);
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
            player.health -= shot.damage;
            particles.sparks(&mut rng, pos, ORANGE, 14, 260.0);
        }
        commands.entity(entity).despawn();
    }
}
