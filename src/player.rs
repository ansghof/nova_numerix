//! Das Spielerschiff: fliegen mit WASD oder Pfeiltasten, feuern mit der Leertaste. Die
//! Bewaffnung wächst mit Powerups und ist am Schiff zu sehen: Laserrohre an der Nase,
//! Raketenwerfer unter den Flügeln, Seitenkanonen schräg an Ober- und Unterseite und bis zu zwei
//! Begleitdrohnen, die dem Schiff folgen.

use bevy::prelude::*;

use crate::boss::BossState;
use crate::level::{LevelStarted, Rewind};
use crate::shipyard::Credits;
use crate::neon::{CYAN, GREEN, Shape, WHITE, polygon, scaled};
use crate::ship::Hull;
use crate::particles::Particles;
use crate::rng::Rng;
use crate::shots;
use crate::sound::play;
use crate::synth::Sfx;
use crate::{GamePhase, Gameplay, HALF_HEIGHT, Playfield, Shake};

/// Schild des Startrumpfs; größere Rümpfe tragen mehr, siehe [`Player::max_health`].
pub const MAX_HEALTH: f32 = 100.0;
pub const MAX_LEVEL: u8 = 5;
pub const MAX_SIDE: u8 = 3;
pub const MAX_DRONES: u8 = 2;
/// Radius der Schutzblase während einer Aufgabe und kurz danach.
pub const BUBBLE_RADIUS: f32 = 70.0;
/// Trefferradius des Schiffs selbst.
pub const HIT_RADIUS: f32 = 16.0;
const SPEED: f32 = 430.0;
const LASER_COOLDOWN: f32 = 0.13;
const MISSILE_COOLDOWN: f32 = 0.9;
const SIDE_COOLDOWN: f32 = 0.3;
/// Drohnen feuern etwas langsamer als das Schiff.
const DRONE_COOLDOWN: f32 = 0.2;
/// Wo die Drohnen relativ zum Schiff fliegen.
const DRONE_OFFSETS: [Vec2; 2] = [Vec2::new(-18.0, 52.0), Vec2::new(-18.0, -52.0)];
/// Wie straff die Drohnen dem Schiff folgen; größer heißt dichter dran.
const DRONE_FOLLOW: f32 = 9.0;
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
            .add_systems(Update, (start_of_level, drones_to_start))
            .add_systems(
                Update,
                (
                    steer,
                    fire.run_if(in_state(GamePhase::Flying)).run_if(|boss: Res<BossState>| !boss.defeated),
                    tick_invulnerability,
                    exhaust,
                    show_weapons,
                    sync_drones,
                    follow_drones,
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
    /// Höchster Schild; hängt vom Rumpf ab.
    pub max_health: f32,
    /// Anteil eines Treffers, der durch die Panzerung kommt.
    pub armor: f32,
    /// Restliche Unverwundbarkeit in echten Sekunden.
    pub invulnerable: f32,
    laser_cooldown: f32,
    missile_cooldown: f32,
    side_cooldown: f32,
    drone_cooldown: f32,
}

impl Player {
    /// Ein Treffer durch Schuss oder Zusammenstoß; die Panzerung schluckt einen Teil.
    pub fn hurt(&mut self, amount: f32) {
        self.health -= amount * self.armor;
    }

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
    /// Seitenkanonen feuern schräg nach oben und unten, gut gegen Geschütze am Rand.
    pub side: u8,
    pub drones: u8,
}

impl Default for Loadout {
    fn default() -> Self {
        Self { laser: 1, missiles: 0, side: 0, drones: 0 }
    }
}

impl Loadout {
    /// Wie viele Waffensysteme an Bord sind; der Laser ist immer dabei.
    pub fn systems(&self) -> usize {
        1 + [self.missiles, self.side, self.drones].iter().filter(|level| **level > 0).count()
    }

    /// Ob ein Waffensystem dieser Stufe noch dazupasst: ausbauen geht immer, ein neues System
    /// braucht einen freien Waffenplatz am Rumpf.
    pub fn fits(&self, current: u8, platforms: usize) -> bool {
        current > 0 || self.systems() < platforms
    }

    /// Kurzbeschreibung für HUD, Werft und Siegerbildschirm.
    pub fn describe(&self) -> String {
        let mut text = format!("Laser {}", self.laser);
        for (level, name) in [(self.missiles, "Raketen"), (self.side, "Seitenkanonen"), (self.drones, "Drohnen")] {
            if level > 0 {
                text += &format!(" · {name} {level}");
            }
        }
        text
    }
}

/// Eine Begleitdrohne; `slot` bestimmt, ob sie über oder unter dem Schiff fliegt.
#[derive(Component)]
pub struct Drone {
    slot: usize,
}

fn drone_shape() -> Shape {
    Shape::new(GREEN)
        .closed(&polygon(4, 10.0, 0.0))
        .closed(&polygon(4, 4.0, std::f32::consts::FRAC_PI_4))
        .open(&[Vec2::new(4.0, 0.0), Vec2::new(14.0, 0.0)])
}

/// Die sichtbaren Waffen am Schiff, neu aufgebaut, sobald sich die Bewaffnung ändert.
#[derive(Component)]
struct WeaponMounts;

fn spawn_player(mut commands: Commands, field: Res<Playfield>) {
    commands.spawn((
        Player {
            health: MAX_HEALTH,
            max_health: MAX_HEALTH,
            armor: 1.0,
            invulnerable: 0.0,
            laser_cooldown: 0.0,
            missile_cooldown: 0.0,
            side_cooldown: 0.0,
            drone_cooldown: 0.0,
        },
        Hull::Spatz.shape(),
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

/// Neues Level: Drohnen springen mit dem Schiff an den Start.
fn drones_to_start(
    mut started: MessageReader<LevelStarted>,
    field: Res<Playfield>,
    mut drones: Query<(&Drone, &mut Transform)>,
) {
    if started.read().last().is_none() {
        return;
    }
    for (drone, mut transform) in &mut drones {
        transform.translation = (start(&field).truncate() + DRONE_OFFSETS[drone.slot]).extend(1.0);
    }
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
    drones: Query<&Transform, (With<Drone>, Without<Player>)>,
) {
    let (mut player, transform) = player.into_inner();
    let dt = time.delta_secs();
    player.laser_cooldown -= dt;
    player.missile_cooldown -= dt;
    player.side_cooldown -= dt;
    player.drone_cooldown -= dt;
    if !keys.pressed(KeyCode::Space) {
        return;
    }
    let pos = transform.translation.truncate();

    if player.laser_cooldown <= 0.0 {
        player.laser_cooldown = LASER_COOLDOWN;
        play(&mut commands, Sfx::Laser);
        let n = loadout.laser as usize;
        for i in 0..n {
            let offset = i as f32 - (n - 1) as f32 / 2.0;
            // Ab Stufe 4 fächern die äußeren Strahlen etwas auf.
            let spread = if n >= 4 { offset * 0.06 } else { 0.0 };
            shots::spawn_laser(&mut commands, pos + Vec2::new(30.0, offset * 9.0), Vec2::from_angle(spread), WHITE);
        }
    }

    // Seitenkanonen: je Stufe ein Schuss mehr nach oben und nach unten, schön aufgefächert.
    if loadout.side > 0 && player.side_cooldown <= 0.0 {
        player.side_cooldown = SIDE_COOLDOWN;
        let n = loadout.side as usize;
        for i in 0..n {
            let angle = (35.0 + (i as f32 - (n - 1) as f32 / 2.0) * 20.0).to_radians();
            for side in [1.0, -1.0] {
                let dir = Vec2::from_angle(angle * side);
                shots::spawn_side_shot(&mut commands, pos + Vec2::new(4.0, side * 14.0), dir);
            }
        }
    }

    if !drones.is_empty() && player.drone_cooldown <= 0.0 {
        player.drone_cooldown = DRONE_COOLDOWN;
        for drone in &drones {
            shots::spawn_laser(&mut commands, drone.translation.truncate() + Vec2::X * 14.0, Vec2::X, GREEN);
        }
    }

    if loadout.missiles > 0 && player.missile_cooldown <= 0.0 {
        player.missile_cooldown = MISSILE_COOLDOWN;
        play(&mut commands, Sfx::Missile);
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
    // Seitenkanonen: schräge Rohre oben und unten am Rumpf, eins je Stufe.
    for i in 0..loadout.side {
        for side in [1.0, -1.0] {
            let x = -4.0 + i as f32 * 6.0;
            shape = shape.open(&[Vec2::new(x, side * 9.0), Vec2::new(x + 7.0, side * 17.0)]);
        }
    }
    commands.spawn((WeaponMounts, shape, Transform::default(), ChildOf(*player)));
}

/// So viele Drohnen, wie die Bewaffnung vorsieht; neue erscheinen direkt am Schiff.
fn sync_drones(
    mut commands: Commands,
    loadout: Res<Loadout>,
    player: Single<&Transform, With<Player>>,
    drones: Query<(Entity, &Drone)>,
) {
    if !loadout.is_changed() {
        return;
    }
    let wanted = loadout.drones as usize;
    for (entity, drone) in &drones {
        if drone.slot >= wanted {
            commands.entity(entity).despawn();
        }
    }
    for (slot, offset) in DRONE_OFFSETS.iter().enumerate().take(wanted) {
        if !drones.iter().any(|(_, d)| d.slot == slot) {
            let pos = player.translation.truncate() + *offset;
            commands.spawn((Drone { slot }, drone_shape(), Transform::from_translation(pos.extend(1.0))));
        }
    }
}

/// Die Drohnen ziehen dem Schiff mit etwas Verzögerung hinterher.
fn follow_drones(
    time: Res<Time>,
    player: Single<&Transform, With<Player>>,
    mut drones: Query<(&Drone, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_secs();
    let follow = 1.0 - (-DRONE_FOLLOW * dt).exp();
    for (drone, mut transform) in &mut drones {
        let target = player.translation.truncate() + DRONE_OFFSETS[drone.slot];
        let pos = transform.translation.truncate().lerp(target, follow);
        transform.translation = pos.extend(1.0);
    }
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

/// Wird das Schiff zerstört, gehen alle Waffen und die noch nicht gesicherten Flug-Credits
/// verloren; ein neues startet links am letzten Checkpoint. Rumpf und Panzerung bleiben.
fn check_death(
    mut loadout: ResMut<Loadout>,
    mut particles: ResMut<Particles>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<Shake>,
    mut credits: ResMut<Credits>,
    mut rewind: MessageWriter<Rewind>,
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
    player.health = player.max_health;
    player.invulnerable = RESPAWN_INVULNERABLE;
    transform.translation = start(&field);
    rewind.write(Rewind);
}
