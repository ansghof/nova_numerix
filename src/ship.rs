//! Rumpf und Panzerung: der Teil des Schiffs, der auch bei einem Verlust bleibt. Der Rumpf
//! bestimmt Form, Schildstärke und wie viele Waffensysteme Platz finden; die Panzerung schluckt
//! einen Teil jedes Treffers und ist als Plattenkranz um den Rumpf zu sehen.

use bevy::prelude::*;

use crate::neon::{CYAN, Shape, scaled};
use crate::difficulty::Difficulty;
use crate::player::Player;
use crate::{GamePhase, Gameplay};

pub const MAX_ARMOR: u8 = 3;
/// So viel jedes Panzerungsstufe von einem Treffer abhält.
const ARMOR_PER_LEVEL: f32 = 0.15;
/// Farbe der Panzerplatten: kühles Stahlblau.
const PLATE: LinearRgba = LinearRgba::rgb(0.9, 1.3, 1.9);

pub struct ShipPlugin;

impl Plugin for ShipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ship>()
            .add_systems(Update, apply_ship)
            .add_systems(Update, regenerate.in_set(Gameplay));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hull {
    Spatz,
    Falke,
    Kondor,
    Phoenix,
}

impl Hull {
    pub const ALL: [Hull; 4] = [Hull::Spatz, Hull::Falke, Hull::Kondor, Hull::Phoenix];

    pub fn index(self) -> u8 {
        Self::ALL.iter().position(|h| *h == self).unwrap_or(0) as u8
    }

    pub fn from_index(index: u8) -> Self {
        Self::ALL.get(index as usize).copied().unwrap_or(Hull::Spatz)
    }

    pub fn next(self) -> Option<Self> {
        Self::ALL.get(self.index() as usize + 1).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Hull::Spatz => "Spatz",
            Hull::Falke => "Falke",
            Hull::Kondor => "Kondor",
            Hull::Phoenix => "Phönix",
        }
    }

    /// Wie viele Waffensysteme (Laser, Raketen, Seitenkanonen, Drohnen) der Rumpf trägt.
    pub fn platforms(self) -> usize {
        match self {
            Hull::Spatz => 2,
            Hull::Falke => 3,
            Hull::Kondor | Hull::Phoenix => 4,
        }
    }

    pub fn max_health(self) -> f32 {
        match self {
            Hull::Spatz => 100.0,
            Hull::Falke => 120.0,
            Hull::Kondor => 140.0,
            Hull::Phoenix => 160.0,
        }
    }

    /// Der Phönix hat einen Schildgenerator, der im Flug langsam nachlädt (Punkte je Sekunde).
    pub fn regeneration(self) -> f32 {
        if self == Hull::Phoenix { 0.4 } else { 0.0 }
    }

    /// Kaufpreis in Quibbos Laden; für so viel muss man über mehrere Besuche sparen.
    pub fn price(self) -> u32 {
        match self {
            Hull::Spatz => 0,
            Hull::Falke => 3000,
            Hull::Kondor => 5000,
            Hull::Phoenix => 8000,
        }
    }

    /// Kurz, was der Rumpf kann, für Laden und Werft.
    pub fn describe(self) -> String {
        let mut text = format!("{} Waffenplätze, Schild {:.0}", self.platforms(), self.max_health());
        if self.regeneration() > 0.0 {
            text += ", Schildgenerator";
        }
        text
    }

    /// Der Umriss des Rumpfs; die Nase zeigt nach rechts.
    fn outline(self) -> Vec<Vec2> {
        let half: &[(f32, f32)] = match self {
            Hull::Spatz => &[(36.0, 0.0), (-6.0, 12.0), (-20.0, 22.0), (-16.0, 7.0), (-24.0, 0.0)],
            Hull::Falke => &[(42.0, 0.0), (10.0, 7.0), (-4.0, 24.0), (-14.0, 26.0), (-10.0, 8.0), (-24.0, 5.0), (-28.0, 0.0)],
            Hull::Kondor => &[
                (40.0, 0.0),
                (14.0, 8.0),
                (0.0, 14.0),
                (-8.0, 30.0),
                (-20.0, 32.0),
                (-16.0, 14.0),
                (-26.0, 8.0),
                (-30.0, 0.0),
            ],
            Hull::Phoenix => &[
                (44.0, 0.0),
                (16.0, 6.0),
                (2.0, 12.0),
                (-6.0, 30.0),
                (-14.0, 24.0),
                (-16.0, 34.0),
                (-24.0, 26.0),
                (-18.0, 12.0),
                (-28.0, 6.0),
                (-34.0, 0.0),
            ],
        };
        // Obere Hälfte und gespiegelt die untere, ohne die Punkte auf der Mittellinie doppelt.
        let top: Vec<Vec2> = half.iter().map(|&(x, y)| Vec2::new(x, y)).collect();
        let bottom = top[1..top.len() - 1].iter().rev().map(|p| Vec2::new(p.x, -p.y));
        top.iter().copied().chain(bottom).collect()
    }

    /// Das ganze Schiff: Umriss, Cockpit und je Rumpf ein paar Zierlinien.
    pub fn shape(self) -> Shape {
        let mut shape = Shape::new(CYAN)
            .closed(&self.outline())
            .closed(&[Vec2::new(16.0, 0.0), Vec2::new(4.0, 4.0), Vec2::new(-2.0, 0.0), Vec2::new(4.0, -4.0)]);
        for side in [1.0, -1.0] {
            shape = match self {
                Hull::Spatz => shape,
                // Kleine Entenflügel vorn
                Hull::Falke => shape.open(&[Vec2::new(20.0, 3.0 * side), Vec2::new(12.0, 12.0 * side)]),
                // Doppelflügel
                Hull::Kondor => shape.open(&[Vec2::new(-2.0, 16.0 * side), Vec2::new(-12.0, 28.0 * side)]),
                // Schwanzfedern
                Hull::Phoenix => shape
                    .open(&[Vec2::new(-28.0, 6.0 * side), Vec2::new(-42.0, 13.0 * side)])
                    .open(&[Vec2::new(-30.0, 3.0 * side), Vec2::new(-44.0, 5.0 * side)]),
            };
        }
        shape
    }
}

/// Rumpf und Panzerung; bleiben beim Verlust des Schiffs erhalten.
#[derive(Resource)]
pub struct Ship {
    pub hull: Hull,
    pub armor: u8,
}

impl Default for Ship {
    fn default() -> Self {
        Self { hull: Hull::Spatz, armor: 0 }
    }
}

impl Ship {
    /// Anteil eines Treffers, der durch die Panzerung kommt.
    pub fn damage_factor(&self) -> f32 {
        1.0 - ARMOR_PER_LEVEL * self.armor as f32
    }

    pub fn describe(&self) -> String {
        if self.armor > 0 {
            format!("{} · Panzerung {}", self.hull.name(), self.armor)
        } else {
            self.hull.name().to_string()
        }
    }

    /// Panzerung in Prozent, für den Laden.
    pub fn armor_percent(armor: u8) -> u32 {
        (ARMOR_PER_LEVEL * armor as f32 * 100.0).round() as u32
    }
}

/// Die Panzerplatten als Kranz um den Rumpf, ein Ring je Stufe.
#[derive(Component)]
struct ArmorPlates;

/// Rumpf oder Panzerung haben sich geändert: Form, Schildgrenze und Platten anpassen.
fn apply_ship(
    mut commands: Commands,
    ship: Res<Ship>,
    difficulty: Res<Difficulty>,
    player: Single<(Entity, &mut Player, &mut Shape)>,
    plates: Query<Entity, With<ArmorPlates>>,
) {
    if !ship.is_changed() && !difficulty.is_changed() {
        return;
    }
    let (entity, mut player, mut shape) = player.into_inner();
    let visible = shape.visible;
    *shape = ship.hull.shape();
    shape.visible = visible;
    player.max_health = ship.hull.max_health();
    player.health = player.health.min(player.max_health);
    // Panzerung und Schwierigkeitsgrad bestimmen zusammen, wie viel ein Treffer kostet.
    player.armor = ship.damage_factor() * difficulty.damage_factor();

    for plate in &plates {
        commands.entity(plate).despawn();
    }
    if ship.armor > 0 {
        // Jede Stufe ein etwas größerer Abklatsch des Umrisses, in Stücke geteilt wie Platten.
        let outline = ship.hull.outline();
        let mut plates = Shape::new(PLATE);
        for level in 0..ship.armor {
            let grow = 1.12 + level as f32 * 0.1;
            for i in 0..outline.len() {
                let a = outline[i] * grow;
                let b = outline[(i + 1) % outline.len()] * grow;
                // Nur das mittlere Stück jeder Kante, so entstehen Lücken zwischen den Platten.
                plates = plates.open(&[a.lerp(b, 0.15), a.lerp(b, 0.85)]);
            }
        }
        plates.color = scaled(PLATE, 0.8);
        commands.spawn((ArmorPlates, plates, Transform::default(), ChildOf(entity)));
    }
}

/// Der Schildgenerator des Phönix lädt im Flug langsam nach.
fn regenerate(time: Res<Time>, ship: Res<Ship>, phase: Res<State<GamePhase>>, mut player: Single<&mut Player>) {
    let rate = ship.hull.regeneration();
    if rate > 0.0 && *phase.get() == GamePhase::Flying && player.health > 0.0 {
        player.health = (player.health + rate * time.delta_secs()).min(player.max_health);
    }
}
