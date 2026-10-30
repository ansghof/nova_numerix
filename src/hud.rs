//! Anzeige am Bildschirmrand: Schild, Punkte, Bewaffnung und Schiffsname.

use bevy::prelude::*;

use crate::player::{Loadout, MAX_HEALTH, Player};

/// Vorerst fest; später kommt der Name aus dem Speicherstand.
const SHIP_NAME: &str = "Sternfalke";
const TEXT: Color = Color::srgb(0.8, 0.9, 1.0);
const SHIELD: Color = Color::srgb(0.3, 0.85, 1.0);
const SHIELD_LOW: Color = Color::srgb(1.0, 0.4, 0.4);

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>().add_systems(Startup, spawn_hud).add_systems(Update, update_hud);
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

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

fn spawn_hud(mut commands: Commands) {
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
    let bar = commands
        .spawn((
            Node { width: Val::Px(220.0), height: Val::Px(10.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(Color::srgb(0.08, 0.1, 0.2)),
            ChildOf(left),
        ))
        .id();
    commands.spawn((
        ShieldFill,
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
        BackgroundColor(SHIELD),
        ChildOf(bar),
    ));
    commands.spawn((ScoreText, Text::new(""), TextColor(TEXT), font(20.0), ChildOf(left)));
    commands.spawn((WeaponText, Text::new(""), TextColor(TEXT), font(15.0), ChildOf(left)));

    commands.spawn((
        Text::new(SHIP_NAME),
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
