//! Das Spielerschiff: fliegen mit WASD oder Pfeiltasten, feuern mit der Leertaste. Die
//! Bewaffnung wächst mit Powerups und ist am Schiff zu sehen.

use bevy::prelude::*;

use crate::boss::BossState;
use crate::level::LevelStarted;
use crate::shipyard::Credits;
use crate::neon::{CYAN, Shape, WHITE, scaled};
use crate::particles::Particles;
use crate::rng::Rng;
use crate::shots;
use crate::{GamePhase, Gameplay, HALF_HEIGHT, Playfield, Shake};

pub const MAX_HEALTH: f32 = 100.0;
pub const MAX_LEVEL: u8 = 5;
/// Radius der Schutzblase während einer Aufgabe und kurz danach.
pub const BUBBLE_RADIUS: f32 = 70.0;
/// Trefferradius des Schiffs selbst.
pub const HIT_RADIUS: f32 = 16.0;
const SPEED: f32 = 430.0;
const LASER_COOLDOWN: f32 = 0.13;
const MISSILE_COOLDOWN: f32 = 0.9;
/// Startpunkt, so weit vom linken Rand entfernt.
const START_FROM_EDGE: f32 = 160.0;

fn start(field: &Playfield) -> Vec3 {
    Vec3::new(-field.half_width + START_FROM_EDGE, 0.0, 1.0)
}
/// Nach einem Verlust des Schiffs bleibt das neue eine Weile unverwundbar.
const RESPAWN_INVULNERABLE: f32 = 2.5;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loadout>()
            .add_systems(Startup, spawn_player)
            .add_systems(PreUpdate, place_at_start.after(crate::update_playfield))
            .add_systems(Update, start_of_level)
            .add_systems(
                Update,
                (
                    steer,
                    fire.run_if(in_state(GamePhase::Flying)).run_if(|boss: Res<BossState>| !boss.defeated),
                    tick_invulnerability,
                    exhaust,
                    show_weapons,
                    draw_bubble,
                    check_death,
                )
                    .in_set(Gameplay),
            );
    }
}

#[derive(Component)]
pub struct Player {
    pub health: f32,
    /// Restliche Unverwundbarkeit in echten Sekunden.
    pub invulnerable: f32,
    laser_cooldown: f32,
    missile_cooldown: f32,
}

impl Player {
    /// Während einer Aufgabe, in der Rückkehr-Phase und solange das Schiff blinkt, kann ihm
    /// nichts passieren.
    pub fn protected(&self, phase: GamePhase) -> bool {
        phase != GamePhase::Flying || self.invulnerable > 0.0
    }
}

/// Die Bewaffnung. Geht verloren, wenn das Schiff zerstört wird.
#[derive(Resource)]
pub struct Loadout {
    pub laser: u8,
    pub missiles: u8,
}

impl Default for Loadout {
    fn default() -> Self {
        Self { laser: 1, missiles: 0 }
    }
}

/// Die sichtbaren Waffen am Schiff, neu aufgebaut, sobald sich die Bewaffnung ändert.
#[derive(Component)]
struct WeaponMounts;

fn ship_shape() -> Shape {
    Shape::new(CYAN)
        .closed(&[
            Vec2::new(36.0, 0.0),
            Vec2::new(-6.0, 12.0),
            Vec2::new(-20.0, 22.0),
            Vec2::new(-16.0, 7.0),
            Vec2::new(-24.0, 0.0),
            Vec2::new(-16.0, -7.0),
            Vec2::new(-20.0, -22.0),
            Vec2::new(-6.0, -12.0),
        ])
        // Cockpit
        .closed(&[Vec2::new(16.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(-2.0, 0.0), Vec2::new(4.0, -4.0)])
}

fn spawn_player(mut commands: Commands, field: Res<Playfield>) {
    commands.spawn((
        Player { health: MAX_HEALTH, invulnerable: 0.0, laser_cooldown: 0.0, missile_cooldown: 0.0 },
        ship_shape(),
        Transform::from_translation(start(&field)),
    ));
}

/// Beim Start ist die Fensterbreite noch unbekannt; sobald sie feststeht, rückt das Schiff an
/// den linken Rand.
fn place_at_start(field: Res<Playfield>, mut player: Single<&mut Transform, With<Player>>, mut frames: Local<u32>) {
    if *frames < 3 {
        *frames += 1;
        player.translation = start(&field);
    }
}

fn start_of_level(
    mut started: MessageReader<LevelStarted>,
    field: Res<Playfield>,
    player: Single<(&mut Player, &mut Transform)>,
) {
    if started.read().last().is_none() {
        return;
    }
    let (mut player, mut transform) = player.into_inner();
    transform.translation = start(&field);
    player.invulnerable = 1.0;
}

fn steer(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    field: Res<Playfield>,
    mut player: Single<&mut Transform, With<Player>>,
) {
    let pressed = |a: KeyCode, b: KeyCode| keys.pressed(a) || keys.pressed(b);
    let mut dir = Vec2::ZERO;
    if pressed(KeyCode::KeyW, KeyCode::ArrowUp) {
        dir.y += 1.0;
    }
    if pressed(KeyCode::KeyS, KeyCode::ArrowDown) {
        dir.y -= 1.0;
    }
    if pressed(KeyCode::KeyA, KeyCode::ArrowLeft) {
        dir.x -= 1.0;
    }
    if pressed(KeyCode::KeyD, KeyCode::ArrowRight) {
        dir.x += 1.0;
    }
    let pos = player.translation.truncate() + dir.normalize_or_zero() * SPEED * time.delta_secs();
    let max = Vec2::new(field.half_width - 30.0, HALF_HEIGHT - 60.0);
    player.translation = pos.clamp(-max, max).extend(player.translation.z);
    // Leicht in die Kurve legen.
    player.rotation = Quat::from_rotation_z(dir.y * 0.08);
}

fn fire(
    mut commands: Commands,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    loadout: Res<Loadout>,
    player: Single<(&mut Player, &Transform)>,
) {
    let (mut player, transform) = player.into_inner();
    let dt = time.delta_secs();
    player.laser_cooldown -= dt;
    player.missile_cooldown -= dt;
    if !keys.pressed(KeyCode::Space) {
        return;
    }
    let pos = transform.translation.truncate();

    if player.laser_cooldown <= 0.0 {
        player.laser_cooldown = LASER_COOLDOWN;
        let n = loadout.laser as usize;
        for i in 0..n {
            let offset = i as f32 - (n - 1) as f32 / 2.0;
            // Ab Stufe 4 fächern die äußeren Strahlen etwas auf.
            let spread = if n >= 4 { offset * 0.06 } else { 0.0 };
            shots::spawn_laser(&mut commands, pos + Vec2::new(30.0, offset * 9.0), Vec2::from_angle(spread));
        }
    }

    if loadout.missiles > 0 && player.missile_cooldown <= 0.0 {
        player.missile_cooldown = MISSILE_COOLDOWN;
        for i in 0..loadout.missiles {
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            let dir = Vec2::new(0.6, side * 0.8 * (1.0 + i as f32 * 0.3)).normalize();
            shots::spawn_missile(&mut commands, pos + Vec2::new(-4.0, side * 24.0), dir);
        }
    }
}

fn tick_invulnerability(
    time: Res<Time<Real>>,
    phase: Res<State<GamePhase>>,
    player: Single<(&mut Player, &mut Shape)>,
) {
    let (mut player, mut shape) = player.into_inner();
    player.invulnerable = (player.invulnerable - time.delta_secs()).max(0.0);
    // Blinken nur nach der Rückkehr; während der Aufgabe zeigt die Blase den Schutz.
    shape.visible = *phase.get() != GamePhase::Flying
        || player.invulnerable <= 0.0
        || (time.elapsed_secs() * 16.0) as u32 % 2 == 0;
}

fn exhaust(mut particles: ResMut<Particles>, mut rng: ResMut<Rng>, player: Single<&Transform, With<Player>>) {
    let pos = player.translation.truncate();
    for y in [-6.0, 6.0] {
        particles.exhaust(&mut rng, pos + Vec2::new(-24.0, y), Vec2::NEG_X, scaled(CYAN, 0.8));
    }
}

fn show_weapons(
    mut commands: Commands,
    loadout: Res<Loadout>,
    player: Single<Entity, With<Player>>,
    mounts: Query<Entity, With<WeaponMounts>>,
) {
    if !loadout.is_changed() {
        return;
    }
    for mount in &mounts {
        commands.entity(mount).despawn();
    }
    let mut shape = Shape::new(scaled(CYAN, 1.3));
    // Jede Laserstufe über der ersten bekommt ein eigenes Rohr an der Nase.
    for i in 1..loadout.laser {
        let y = (i as f32 - loadout.laser as f32 / 2.0) * 9.0;
        shape = shape.open(&[Vec2::new(8.0, y), Vec2::new(26.0, y)]);
    }
    // Raketenwerfer unter den Flügeln, mit einem Rohr je Stufe.
    if loadout.missiles > 0 {
        for side in [1.0, -1.0] {
            let y = side * 24.0;
            shape = shape.closed(&[
                Vec2::new(-14.0, y - 4.0),
                Vec2::new(6.0, y - 4.0),
                Vec2::new(10.0, y),
                Vec2::new(6.0, y + 4.0),
                Vec2::new(-14.0, y + 4.0),
            ]);
            for i in 0..loadout.missiles {
                let x = -12.0 + i as f32 * 4.0;
                shape = shape.open(&[Vec2::new(x, y - 2.0), Vec2::new(x, y + 2.0)]);
            }
        }
    }
    commands.spawn((WeaponMounts, shape, Transform::default(), ChildOf(*player)));
}

fn draw_bubble(
    mut gizmos: Gizmos,
    time: Res<Time<Real>>,
    phase: Res<State<GamePhase>>,
    player: Single<(&Player, &Transform)>,
) {
    let (player, transform) = player.into_inner();
    if !player.protected(*phase.get()) {
        return;
    }
    let pulse = 0.6 + 0.25 * (time.elapsed_secs() * 6.0).sin();
    let pos = transform.translation.truncate();
    gizmos.circle_2d(pos, BUBBLE_RADIUS, scaled(CYAN, pulse)).resolution(64);
    gizmos.circle_2d(pos, BUBBLE_RADIUS - 5.0, scaled(WHITE, pulse * 0.15)).resolution(64);
}

/// Wird das Schiff zerstört, gehen alle Waffen verloren; ein neues startet links.
fn check_death(
    mut loadout: ResMut<Loadout>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<Shake>,
    mut credits: ResMut<Credits>,
    field: Res<Playfield>,
    player: Single<(&mut Player, &mut Transform, &Shape)>,
) {
    let (mut player, mut transform, shape) = player.into_inner();
    if player.health > 0.0 {
        return;
    }
    info!("Schiff zerstört");
    particles.explode(&mut rng, shape, &transform, 2.0);
    shake.0 = 12.0;
    *loadout = Loadout::default();
    credits.flight = 0;
    player.health = MAX_HEALTH;
    player.invulnerable = RESPAWN_INVULNERABLE;
    transform.translation = start(&field);
}
