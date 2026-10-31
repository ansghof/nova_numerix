//! Die Superwaffe: Q drücken, eine Aufgabe lösen, und eine Schockwelle trifft alle Gegner auf
//! dem Bildschirm. Je schneller die Antwort, desto stärker die Welle. Sie startet noch in der
//! Zeitlupe und breitet sich mit dem Hochfahren der Welt immer schneller aus.

use bevy::prelude::*;

use crate::Shake;
use crate::boss::{Boss, SUPER_MULTIPLIER};
use crate::enemy::Enemy;
use crate::neon::{CYAN, MAGENTA, WHITE, scaled};
use crate::particles::Particles;
use crate::player::Player;
use crate::rng::Rng;
use crate::shots::Hostile;
use crate::task::PendingTasks;
use crate::{GamePhase, Gameplay, HALF_HEIGHT, Playfield};

pub const CHARGES_PER_LEVEL: u8 = 3;
/// Schaden einer Fehlzündung oder einer zu langsamen Antwort.
const MIN_DAMAGE: f32 = 2.0;
/// Schaden einer sofortigen richtigen Antwort. Bosse bekommen später ein Vielfaches davon ab.
const MAX_DAMAGE: f32 = 14.0;
/// Bis hierhin reicht jede Welle: von jeder Schiffsposition aus über den ganzen Bildschirm.
const MAX_RADIUS: f32 = 1500.0;

pub struct SuperWeaponPlugin;

impl Plugin for SuperWeaponPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FireSuper>()
            .insert_resource(SuperWeapon { charges: CHARGES_PER_LEVEL, bought: 0 })
            .add_systems(Update, refill)
            .add_systems(Startup, spawn_flash)
            .add_systems(
                Update,
                (trigger.run_if(in_state(GamePhase::Flying)), launch, expand, draw, fade_flash).chain().in_set(Gameplay),
            );
    }
}

#[derive(Resource)]
pub struct SuperWeapon {
    pub charges: u8,
    /// Im Shop gekaufte Zusatzladungen für den nächsten Flug.
    pub bought: u8,
}

/// Jedes Level beginnt mit vollen Ladungen, dazu was im Shop gekauft wurde.
fn refill(mut started: MessageReader<crate::level::LevelStarted>, mut weapon: ResMut<SuperWeapon>) {
    if started.read().last().is_some() {
        weapon.charges = CHARGES_PER_LEVEL + weapon.bought;
        weapon.bought = 0;
    }
}

/// Zündet die Superwaffe. Stufe 3 bei sehr schneller, 2 bei richtiger Antwort, 1 bei
/// Zeitablauf oder Fehlzündung; `power` ist der Anteil der Restzeit.
#[derive(Message)]
pub struct FireSuper {
    pub tier: u8,
    pub power: f32,
}

#[derive(Component)]
struct Shockwave {
    tier: u8,
    radius: f32,
    speed: f32,
    damage: f32,
    hit: Vec<Entity>,
}

/// Weißer Blitz über dem ganzen Bild; die Deckkraft klingt in Echtzeit ab.
#[derive(Component)]
struct ScreenFlash(f32);

fn spawn_flash(mut commands: Commands) {
    commands.spawn((
        ScreenFlash(0.0),
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() },
        BackgroundColor(Color::NONE),
        GlobalZIndex(20),
    ));
}

fn trigger(keys: Res<ButtonInput<KeyCode>>, mut weapon: ResMut<SuperWeapon>, mut pending: ResMut<PendingTasks>) {
    if keys.just_pressed(KeyCode::KeyQ) && weapon.charges > 0 && !pending.super_weapon {
        weapon.charges -= 1;
        pending.super_weapon = true;
    }
}

fn launch(
    mut commands: Commands,
    mut messages: MessageReader<FireSuper>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<Shake>,
    mut flash: Single<&mut ScreenFlash>,
    player: Single<&Transform, With<Player>>,
) {
    for fire in messages.read() {
        let at = player.translation.truncate();
        let (speed, damage, flash_alpha, shake_strength, sparks) = match fire.tier {
            3 => (1700.0, MAX_DAMAGE, 0.7, 16.0, 60),
            2 => (1300.0, MIN_DAMAGE + (MAX_DAMAGE - MIN_DAMAGE) * fire.power.max(0.25), 0.35, 8.0, 30),
            _ => (900.0, MIN_DAMAGE, 0.0, 2.0, 8),
        };
        commands.spawn((
            Shockwave { tier: fire.tier, radius: 0.0, speed, damage, hit: Vec::new() },
            Transform::from_translation(at.extend(0.0)),
        ));
        particles.sparks(&mut rng, at, WHITE, sparks, 500.0);
        flash.0 = flash.0.max(flash_alpha);
        shake.0 = shake.0.max(shake_strength);
    }
}

fn expand(
    mut commands: Commands,
    time: Res<Time>,
    field: Res<Playfield>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut waves: Query<(Entity, &mut Shockwave, &Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Shockwave>>,
    shots: Query<(Entity, &Transform), (With<Hostile>, Without<Shockwave>)>,
    mut bosses: Query<(Entity, &mut Boss, &Transform), (Without<Shockwave>, Without<Enemy>)>,
) {
    let dt = time.delta_secs();
    for (entity, mut wave, transform) in &mut waves {
        let center = transform.translation.truncate();
        wave.radius += wave.speed * dt;
        if wave.radius > MAX_RADIUS {
            commands.entity(entity).despawn();
            continue;
        }

        for (enemy_entity, mut enemy, enemy_transform) in &mut enemies {
            let pos = enemy_transform.translation.truncate();
            // Nur was auf dem Bildschirm ist, wird getroffen.
            let on_screen = pos.x.abs() < field.half_width + 20.0 && pos.y.abs() < HALF_HEIGHT + 20.0;
            if !on_screen || wave.hit.contains(&enemy_entity) || pos.distance(center) > wave.radius {
                continue;
            }
            wave.hit.push(enemy_entity);
            enemy.health -= wave.damage;
            enemy.flash = 1.0;
            particles.sparks(&mut rng, pos, WHITE, 8, 300.0);
        }

        // Bosse sind groß und stehen am Rand: Sie zählen, sobald sie auf dem Bildschirm sind.
        for (boss_entity, mut boss, boss_transform) in &mut bosses {
            let pos = boss_transform.translation.truncate();
            if !boss.alive() || pos.x > field.half_width + 100.0 || wave.hit.contains(&boss_entity) || pos.distance(center) > wave.radius {
                continue;
            }
            wave.hit.push(boss_entity);
            let before = boss.health;
            boss.health -= wave.damage * SUPER_MULTIPLIER;
            if before > 0.0 && boss.health <= 0.0 && wave.tier == 3 {
                boss.super_finish = true;
            }
            particles.sparks(&mut rng, pos, WHITE, 30, 450.0);
        }

        // Starke Wellen fegen auch feindliche Schüsse weg.
        if wave.tier >= 2 {
            for (shot, shot_transform) in &shots {
                if shot_transform.translation.truncate().distance(center) < wave.radius {
                    commands.entity(shot).despawn();
                }
            }
        }

        // Funken, die von der Wellenfront nach außen sprühen.
        let count = match wave.tier {
            3 => 10,
            2 => 4,
            _ => 0,
        };
        for _ in 0..count {
            let dir = rng.direction();
            particles.sparks(&mut rng, center + dir * wave.radius, CYAN, 1, 200.0);
        }
    }
}

fn draw(mut gizmos: Gizmos, waves: Query<(&Shockwave, &Transform)>) {
    for (wave, transform) in &waves {
        let center = transform.translation.truncate();
        let fade = 1.0 - wave.radius / MAX_RADIUS;
        let rings: &[(f32, LinearRgba)] = match wave.tier {
            3 => &[(0.0, WHITE), (12.0, CYAN), (28.0, MAGENTA), (46.0, scaled(CYAN, 0.5))],
            2 => &[(0.0, CYAN), (14.0, scaled(WHITE, 0.6))],
            _ => &[(0.0, scaled(CYAN, 0.5))],
        };
        for (offset, color) in rings {
            let radius = wave.radius - offset;
            if radius > 0.0 {
                gizmos.circle_2d(center, radius, scaled(*color, fade)).resolution(96);
            }
        }
    }
}

fn fade_flash(time: Res<Time<Real>>, mut flash: Single<(&mut ScreenFlash, &mut BackgroundColor)>) {
    let (flash, background) = &mut *flash;
    flash.0 = (flash.0 - time.delta_secs() * 2.5).max(0.0);
    background.0 = Color::srgba(0.85, 0.95, 1.0, flash.0);
}
