//! Sterne als kleine Bilder: gefüllt für erreichte, als Umriss für noch offene. Als Bild statt als
//! Schriftzeichen, weil nicht jede Schrift ★ und ☆ kennt (die im Browser mitgelieferte nicht).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const SIZE: usize = 64;
const GOLD: Vec3 = Vec3::new(1.0, 0.82, 0.3);
const DIM: Vec3 = Vec3::new(0.55, 0.62, 0.75);

pub struct StarsPlugin;

impl Plugin for StarsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, draw_stars);
    }
}

#[derive(Resource)]
pub struct StarArt {
    pub full: Handle<Image>,
    pub empty: Handle<Image>,
}

/// Abstand eines Punkts zum Rand eines Fünfecksterns; negativ innen.
fn star_distance(p: Vec2) -> f32 {
    let corners: Vec<Vec2> = (0..10)
        .map(|i| {
            let angle = std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let radius = if i % 2 == 0 { 28.0 } else { 12.0 };
            Vec2::from_angle(angle) * radius
        })
        .collect();
    let mut inside = false;
    let mut distance = f32::MAX;
    for i in 0..corners.len() {
        let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
        distance = distance.min(p.distance(a + ab * t));
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            inside = !inside;
        }
    }
    if inside { -distance } else { distance }
}

fn render(filled: bool) -> Vec<u8> {
    let mut data = vec![0u8; SIZE * SIZE * 4];
    let color = if filled { GOLD } else { DIM };
    for y in 0..SIZE {
        for x in 0..SIZE {
            // Bildzeilen laufen von oben nach unten, der Stern soll mit der Spitze nach oben stehen.
            let p = Vec2::new(x as f32 + 0.5 - SIZE as f32 / 2.0, SIZE as f32 / 2.0 - y as f32 - 0.5);
            let d = star_distance(p);
            let alpha = if filled {
                (0.5 - d).clamp(0.0, 1.0)
            } else {
                // Nur ein Umriss, zwei Pixel breit.
                (1.5 - d.abs()).clamp(0.0, 1.0)
            };
            let i = (y * SIZE + x) * 4;
            data[i] = (color.x * 255.0) as u8;
            data[i + 1] = (color.y * 255.0) as u8;
            data[i + 2] = (color.z * 255.0) as u8;
            data[i + 3] = (alpha * 255.0) as u8;
        }
    }
    data
}

fn draw_stars(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = |filled| {
        images.add(Image::new(
            Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            render(filled),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ))
    };
    let full = image(true);
    let empty = image(false);
    commands.insert_resource(StarArt { full, empty });
}

/// Ein einzelner Stern als UI-Bild.
pub fn star(commands: &mut Commands, parent: Entity, art: &StarArt, filled: bool, size: f32) {
    let image = if filled { art.full.clone() } else { art.empty.clone() };
    commands.spawn((
        Node { width: Val::Px(size), height: Val::Px(size), ..default() },
        ImageNode { image, ..default() },
        ChildOf(parent),
    ));
}

/// Drei Sterne nebeneinander, gefüllt nach den Bits von `mask`.
pub fn star_row(commands: &mut Commands, parent: Entity, art: &StarArt, mask: u8, size: f32) {
    let row = commands
        .spawn((Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(2.0), ..default() }, ChildOf(parent)))
        .id();
    for i in 0..3 {
        star(commands, row, art, mask & (1 << i) != 0, size);
    }
}
