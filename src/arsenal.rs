//! Die schweren Waffen aus Quibbos späterem Sortiment.
//!
//! - **Plasmawerfer** (nach Boss 3): eine langsame, breite Welle, die durch Gegner hindurchgeht
//!   und jeden nur einmal trifft. Schilde halten sie auf.
//! - **Blitzbogen** (nach Boss 4): springt zum nächsten Gegner und von dort weiter. Er springt
//!   über Schilde hinweg und ist damit das Mittel gegen Bits und Legionäre.
//!
//! Beide feuern mit der Leertaste, zusammen mit dem Laser.

use bevy::prelude::*;

use crate::boss::BossState;
use crate::enemy::Enemy;
use crate::particles::Particles;
use crate::player::{Loadout, Player};
use crate::rng::Rng;
use crate::shots;
use crate::sound::play;
use crate::synth::Sfx;
use crate::{GamePhase, Gameplay, HALF_HEIGHT, Playfield};

pub const MAX_PLASMA: u8 = 3;
pub const MAX_LIGHTNING: u8 = 3;
/// Leuchtfarbe des Blitzbogens: helles Eisblau.
pub const LIGHTNING: LinearRgba = LinearRgba::rgb(1.6, 2.6, 4.0);
/// So weit reicht der erste Sprung vom Schiff aus …
const LIGHTNING_REACH: f32 = 620.0;
/// … und so weit jeder weitere von Gegner zu Gegner.
const LIGHTNING_JUMP: f32 = 190.0;
/// So lange steht ein Blitz sichtbar da.
const BOLT_TIME: f32 = 0.25;

pub struct ArsenalPlugin;

impl Plugin for ArsenalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cooldowns>().add_systems(
            Update,
            (
                (fire_plasma, fire_lightning)
                    .run_if(in_state(GamePhase::Flying))
                    .run_if(|boss: Res<BossState>| !boss.defeated),
                draw_bolts,
            )
                .in_set(Gameplay),
        );
    }
}

#[derive(Resource, Default)]
struct Cooldowns {
    plasma: f32,
    lightning: f32,
}

/// Ein sichtbarer Blitz entlang dieser Punkte; flackert, bis er erlischt.
#[derive(Component)]
struct Bolt {
    points: Vec<Vec2>,
    life: f32,
}

fn firing(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::Space)
}

fn fire_plasma(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    loadout: Res<Loadout>,
    mut cooldowns: ResMut<Cooldowns>,
    player: Single<&Transform, With<Player>>,
) {
    cooldowns.plasma -= time.delta_secs();
    if loadout.plasma == 0 || cooldowns.plasma > 0.0 || !firing(&keys) {
        return;
    }
    // Höhere Stufen feuern öfter und breitere, stärkere Wellen.
    cooldowns.plasma = [1.1, 0.9, 0.75][(loadout.plasma - 1) as usize];
    shots::spawn_plasma(&mut commands, player.translation.truncate() + Vec2::new(30.0, 0.0), loadout.plasma);
    play(&mut commands, Sfx::Plasma);
}

fn fire_lightning(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    loadout: Res<Loadout>,
    field: Res<Playfield>,
    mut cooldowns: ResMut<Cooldowns>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    player: Single<&Transform, With<Player>>,
    mut enemies: Query<(Entity, &mut Enemy, &Transform), Without<Player>>,
) {
    cooldowns.lightning -= time.delta_secs();
    if loadout.lightning == 0 || cooldowns.lightning > 0.0 || !firing(&keys) {
        return;
    }
    let start = player.translation.truncate() + Vec2::new(20.0, 0.0);
    // Ziele: alles Lebende auf dem Bildschirm.
    let candidates: Vec<(Entity, Vec2)> = enemies
        .iter()
        .filter(|(_, enemy, _)| enemy.health > 0.0)
        .map(|(entity, _, transform)| (entity, transform.translation.truncate()))
        .filter(|(_, pos)| pos.x.abs() < field.half_width && pos.y.abs() < HALF_HEIGHT)
        .collect();
    let jumps = 2 + loadout.lightning as usize;
    let mut chain: Vec<(Entity, Vec2)> = Vec::new();
    let mut from = start;
    let mut reach = LIGHTNING_REACH;
    for _ in 0..jumps {
        let next = candidates
            .iter()
            .filter(|(entity, _)| !chain.iter().any(|(hit, _)| hit == entity))
            .map(|(entity, pos)| (*entity, *pos, pos.distance(from)))
            .filter(|(_, _, distance)| *distance < reach)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((entity, pos, _)) = next else {
            break;
        };
        chain.push((entity, pos));
        from = pos;
        reach = LIGHTNING_JUMP;
    }
    if chain.is_empty() {
        // Nichts in Reichweite: kurz warten und erneut suchen, ohne die volle Ladezeit.
        cooldowns.lightning = 0.1;
        return;
    }
    cooldowns.lightning = [0.9, 0.75, 0.6][(loadout.lightning - 1) as usize];
    for (entity, pos) in &chain {
        // Der Blitz springt über Schilde: Er trifft direkt.
        if let Ok((_, mut enemy, _)) = enemies.get_mut(*entity) {
            enemy.health -= 1.0;
            enemy.flash = 1.0;
        }
        particles.sparks(&mut rng, *pos, LIGHTNING, 8, 260.0);
    }
    let points = std::iter::once(start).chain(chain.iter().map(|(_, pos)| *pos)).collect();
    debug!("Blitz trifft {} Gegner", chain.len());
    commands.spawn(Bolt { points, life: BOLT_TIME });
    play(&mut commands, Sfx::Zap);
}

/// Zeichnet die Blitze als zitternde Zickzacklinien, jedes Frame neu.
fn draw_bolts(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut rng: ResMut<Rng>,
    mut gizmos: Gizmos,
    mut bolts: Query<(Entity, &mut Bolt)>,
) {
    for (entity, mut bolt) in &mut bolts {
        bolt.life -= time.delta_secs();
        if bolt.life <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let strength = bolt.life / BOLT_TIME;
        for pair in bolt.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let normal = (b - a).perp().normalize_or_zero();
            // Zwei Stränge: ein heller, fast weißer Kern und ein bläulicher, der stärker zittert.
            for (jitter, color) in [(8.0, LinearRgba::rgb(4.0, 4.5, 5.0)), (16.0, LIGHTNING)] {
                let mut zigzag = vec![a];
                for i in 1..7 {
                    let t = i as f32 / 7.0;
                    zigzag.push(a.lerp(b, t) + normal * rng.range(-jitter, jitter));
                }
                zigzag.push(b);
                let faded = LinearRgba::new(color.red * strength, color.green * strength, color.blue * strength, 1.0);
                gizmos.linestrip_2d(zigzag, faded);
            }
        }
    }
}
