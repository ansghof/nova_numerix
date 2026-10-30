//! Anzeige am Bildschirmrand: Schild, Punkte, Bewaffnung, Superwaffen-Ladungen,
//! Level-Fortschritt und Schiffsname.

use bevy::prelude::*;

use crate::level::{CHECKPOINT_SHARE, LevelProgress};
use crate::player::{Loadout, MAX_HEALTH, Player};
use crate::save::ShipName;
use crate::superweapon::{CHARGES_PER_LEVEL, SuperWeapon};

const TEXT: Color = Color::srgb(0.8, 0.9, 1.0);
const DIM: Color = Color::srgb(0.08, 0.1, 0.2);
const SHIELD: Color = Color::srgb(0.3, 0.85, 1.0);
const SHIELD_LOW: Color = Color::srgb(1.0, 0.4, 0.4);
const CHARGE: Color = Color::srgb(1.0, 0.85, 0.35);
const PROGRESS: Color = Color::srgb(0.75, 0.4, 1.0);
const BOSS: Color = Color::srgb(1.0, 0.4, 0.5);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .add_systems(Startup, spawn_hud)
            .add_systems(Update, (update_hud, update_charges, update_progress, update_ship_name));
    }
}

#[derive(Resource, Default)]
pub struct Score(pub u32);

#[derive(Component)]
struct ShieldFill;

#[derive(Component)]
struct ScoreText;

#[derive(Component)]
struct WeaponText;

/// Ein Kästchen pro Superwaffen-Ladung.
#[derive(Component)]
struct ChargePip(u8);

#[derive(Component)]
struct ProgressFill;

#[derive(Component)]
struct ShipNameText;

#[derive(Component)]
struct BossLabel;

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

fn bar(commands: &mut Commands, parent: Entity, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node { width: Val::Px(width), height: Val::Px(height), border_radius: BorderRadius::all(Val::Px(height / 2.0)), ..default() },
            BackgroundColor(DIM),
            ChildOf(parent),
        ))
        .id()
}

fn fill(height: f32, color: Color) -> (Node, BackgroundColor) {
    (
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(height / 2.0)), ..default() },
        BackgroundColor(color),
    )
}

fn spawn_hud(mut commands: Commands) {
    // Links oben: Schild, Punkte, Waffen, Superwaffe
    let left = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Px(12.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let shield = bar(&mut commands, left, 220.0, 10.0);
    commands.spawn((ShieldFill, fill(10.0, SHIELD), ChildOf(shield)));
    commands.spawn((ScoreText, Text::new(""), TextColor(TEXT), font(20.0), ChildOf(left)));
    commands.spawn((WeaponText, Text::new(""), TextColor(TEXT), font(15.0), ChildOf(left)));
    let charges = commands
        .spawn((
            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() },
            ChildOf(left),
        ))
        .id();
    commands.spawn((Text::new("Superwaffe [Q]"), TextColor(TEXT), font(15.0), ChildOf(charges)));
    for i in 0..CHARGES_PER_LEVEL {
        commands.spawn((
            ChargePip(i),
            Node { width: Val::Px(12.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() },
            BackgroundColor(CHARGE),
            ChildOf(charges),
        ));
    }

    // Oben in der Mitte: wie weit es noch bis zum Boss ist
    let top = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Px(14.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let progress = bar(&mut commands, top, 360.0, 8.0);
    commands.spawn((ProgressFill, fill(8.0, PROGRESS), ChildOf(progress)));
    // Markierung für den Checkpoint
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(CHECKPOINT_SHARE * 100.0),
            top: Val::Px(-4.0),
            width: Val::Px(3.0),
            height: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(TEXT),
        ChildOf(progress),
    ));
    commands.spawn((BossLabel, Text::new("Boss"), TextColor(BOSS), font(15.0), ChildOf(top)));

    commands.spawn((
        ShipNameText,
        Text::new(""),
        TextColor(SHIELD),
        font(18.0),
        Node { position_type: PositionType::Absolute, right: Val::Px(16.0), top: Val::Px(12.0), ..default() },
    ));
}

fn update_hud(
    score: Res<Score>,
    loadout: Res<Loadout>,
    player: Single<&Player>,
    mut fill: Single<(&mut Node, &mut BackgroundColor), With<ShieldFill>>,
    mut score_text: Single<&mut Text, (With<ScoreText>, Without<WeaponText>)>,
    mut weapon_text: Single<&mut Text, (With<WeaponText>, Without<ScoreText>)>,
) {
    let share = (player.health / MAX_HEALTH).clamp(0.0, 1.0);
    fill.0.width = Val::Percent(share * 100.0);
    fill.1.0 = if share > 0.3 { SHIELD } else { SHIELD_LOW };
    score_text.0 = format!("{} Punkte", score.0);
    let mut weapons = format!("Laser {}", loadout.laser);
    if loadout.missiles > 0 {
        weapons += &format!(" · Raketen {}", loadout.missiles);
    }
    weapon_text.0 = weapons;
}

fn update_charges(weapon: Res<SuperWeapon>, mut pips: Query<(&ChargePip, &mut BackgroundColor)>) {
    for (pip, mut color) in &mut pips {
        color.0 = if pip.0 < weapon.charges { CHARGE } else { DIM };
    }
}

fn update_progress(
    time: Res<Time<Real>>,
    progress: Res<LevelProgress>,
    mut fill: Single<&mut Node, With<ProgressFill>>,
    mut label: Single<(&mut Text, &mut TextColor), With<BossLabel>>,
) {
    fill.width = Val::Percent(progress.share() * 100.0);
    if progress.boss_ahead() {
        label.0.0 = "Boss voraus!".into();
        // Blinkt, damit man es mitbekommt.
        let on = (time.elapsed_secs() * 3.0) as u32 % 2 == 0;
        label.1.0 = if on { BOSS } else { TEXT };
    } else if label.0.0 != "Boss" {
        label.0.0 = "Boss".into();
        label.1.0 = BOSS;
    }
}

fn update_ship_name(name: Res<ShipName>, mut text: Single<&mut Text, With<ShipNameText>>) {
    if name.is_changed() || text.0.is_empty() {
        text.0 = name.0.clone();
    }
}
