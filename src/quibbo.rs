//! Quibbo, der Rechenkrämer, als Neon-Figur: ein grüner Händler mit vier Augen, zwei Antennen
//! und goldener Fliege. Er wird beim Start einmal in zwei Texturen gezeichnet, mit offenen und
//! mit geschlossenen Augen; die Werft wechselt zwischen beiden, damit er blinzelt.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

const SIZE: usize = 256;
const BODY: Vec3 = Vec3::new(0.35, 1.0, 0.6);
const EYE: Vec3 = Vec3::new(0.95, 1.0, 1.0);
const GOLD: Vec3 = Vec3::new(1.0, 0.8, 0.25);

pub struct QuibboPlugin;

impl Plugin for QuibboPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, draw_quibbo);
    }
}

#[derive(Resource)]
pub struct QuibboArt {
    pub open: Handle<Image>,
    pub blink: Handle<Image>,
}

/// Abstand eines Punkts zu einer Strecke.
fn segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Abstand zum Rand einer Ellipse, näherungsweise; negativ innen.
fn ellipse(p: Vec2, center: Vec2, radius: Vec2) -> f32 {
    let q = (p - center) / radius;
    (q.length() - 1.0) * radius.min_element()
}

/// Leuchtende Linie: heller Kern, weiches Glühen außen herum.
fn glow(distance: f32, width: f32) -> f32 {
    let d = distance.abs();
    if d < width { 1.0 } else { (-(d - width) / 5.0).exp() * 0.55 }
}

fn render(eyes_open: bool) -> Vec<u8> {
    let mut data = vec![0u8; SIZE * SIZE * 4];
    let eye_centers = [Vec2::new(92.0, 142.0), Vec2::new(116.0, 128.0), Vec2::new(140.0, 128.0), Vec2::new(164.0, 142.0)];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
            let mut color = Vec3::ZERO;
            let mut alpha: f32 = 0.0;

            // Körper: gefüllt dunkel, Rand leuchtend
            let body = ellipse(p, Vec2::new(128.0, 150.0), Vec2::new(86.0, 72.0));
            if body < 0.0 {
                alpha = alpha.max(0.85);
                color += Vec3::new(0.02, 0.08, 0.06);
            }
            let rim = glow(body, 2.5);
            color += BODY * rim;
            alpha = alpha.max(rim);

            // Antennen mit Kugeln
            for (base, tip) in [(Vec2::new(104.0, 86.0), Vec2::new(78.0, 34.0)), (Vec2::new(152.0, 86.0), Vec2::new(178.0, 34.0))] {
                let line = glow(segment(p, base, tip), 1.8);
                let ball = glow(p.distance(tip) - 8.0, 2.0);
                color += BODY * line + GOLD * ball;
                alpha = alpha.max(line).max(ball);
            }

            // Vier Augen, offen oder als Strich
            for center in eye_centers {
                let shape = if eyes_open {
                    let white = glow(p.distance(center) - 12.0, 2.0);
                    let pupil = if p.distance(center + Vec2::new(2.0, 2.0)) < 5.0 { 1.0 } else { 0.0 };
                    color += EYE * white + Vec3::splat(0.2) * pupil;
                    white.max(pupil)
                } else {
                    let lid = glow(segment(p, center - Vec2::X * 11.0, center + Vec2::X * 11.0), 2.0);
                    color += EYE * lid;
                    lid
                };
                alpha = alpha.max(shape);
            }

            // Breites Händlerlächeln
            let mouth_center = Vec2::new(128.0, 168.0);
            let mouth = if p.y > mouth_center.y + 10.0 { glow(p.distance(mouth_center) - 28.0, 2.0) } else { 0.0 };
            color += BODY * mouth;
            alpha = alpha.max(mouth);

            // Goldene Fliege unter dem Körper
            let bow = {
                let left = segment(p, Vec2::new(128.0, 228.0), Vec2::new(106.0, 216.0)).min(segment(p, Vec2::new(106.0, 216.0), Vec2::new(106.0, 240.0)));
                let right = segment(p, Vec2::new(128.0, 228.0), Vec2::new(150.0, 216.0)).min(segment(p, Vec2::new(150.0, 216.0), Vec2::new(150.0, 240.0)));
                let closing = segment(p, Vec2::new(106.0, 240.0), Vec2::new(128.0, 228.0)).min(segment(p, Vec2::new(150.0, 240.0), Vec2::new(128.0, 228.0)));
                glow(left.min(right).min(closing), 1.8)
            };
            color += GOLD * bow;
            alpha = alpha.max(bow);

            let i = (y * SIZE + x) * 4;
            let c = color.min(Vec3::ONE);
            data[i] = (c.x * 255.0) as u8;
            data[i + 1] = (c.y * 255.0) as u8;
            data[i + 2] = (c.z * 255.0) as u8;
            data[i + 3] = (alpha.min(1.0) * 255.0) as u8;
        }
    }
    data
}

fn draw_quibbo(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = |eyes_open| {
        images.add(Image::new(
            Extent3d { width: SIZE as u32, height: SIZE as u32, depth_or_array_layers: 1 },
            TextureDimension::D2,
            render(eyes_open),
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        ))
    };
    let open = image(true);
    let blink = image(false);
    commands.insert_resource(QuibboArt { open, blink });
}
