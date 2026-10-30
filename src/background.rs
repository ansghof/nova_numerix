//! Hintergrund in drei Parallax-Ebenen: ferne Sterne, nahe Sterne und ein Gitter am Rand des
//! Spielfelds, das am schnellsten vorbeizieht.

use bevy::prelude::*;

use crate::rng::Rng;
use crate::HALF_HEIGHT;
use crate::level::{Backdrop, CurrentLevel};

/// Geschwindigkeit der drei Ebenen in Welt-Einheiten pro Sekunde.
const LAYER_SPEED: [f32; 3] = [18.0, 55.0, 140.0];
const STARS_PER_LAYER: usize = 110;
const GRID_SPACING: f32 = 80.0;
/// Halbe Breite des Sternenfelds: reicht auch für Monitore bis 32:9.
const SPAN: f32 = 1350.0;

pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_stars).add_systems(Update, (scroll, draw).chain());
    }
}

struct Star {
    pos: Vec2,
    layer: usize,
}

#[derive(Resource)]
struct Starfield {
    stars: Vec<Star>,
    grid_offset: f32,
}

fn spawn_stars(mut commands: Commands, mut rng: ResMut<Rng>) {
    let stars = (0..3)
        .flat_map(|layer| (0..STARS_PER_LAYER).map(move |_| layer))
        .map(|layer| Star {
            pos: Vec2::new(rng.range(-SPAN, SPAN), rng.range(-HALF_HEIGHT, HALF_HEIGHT)),
            layer,
        })
        .collect();
    commands.insert_resource(Starfield { stars, grid_offset: 0.0 });
}

fn scroll(time: Res<Time>, mut field: ResMut<Starfield>) {
    let dt = time.delta_secs();
    for star in &mut field.stars {
        star.pos.x -= LAYER_SPEED[star.layer] * dt;
        if star.pos.x < -SPAN {
            star.pos.x += SPAN * 2.0;
        }
    }
    field.grid_offset = (field.grid_offset + LAYER_SPEED[2] * dt) % GRID_SPACING;
}

fn draw(mut gizmos: Gizmos, time: Res<Time>, field: Res<Starfield>, level: Res<CurrentLevel>) {
    for star in &field.stars {
        let brightness = [0.25, 0.55, 1.1][star.layer];
        let length = [1.5, 3.0, 7.0][star.layer];
        let color = LinearRgba::rgb(0.7 * brightness, 0.8 * brightness, 1.2 * brightness);
        gizmos.line_2d(star.pos, star.pos + Vec2::X * length, color);
    }

    match level.def().backdrop {
        Backdrop::Stars => {}
        Backdrop::GasGiant => {
            // Ein riesiger Planet ragt von unten ins Bild, mit Wolkenbändern.
            let center = Vec2::new(250.0, -HALF_HEIGHT - 330.0);
            let radius = 560.0;
            let rim = LinearRgba::rgb(0.55, 0.22, 0.05);
            gizmos.circle_2d(center, radius, rim).resolution(160);
            for (i, dy) in [430.0, 360.0, 280.0, 190.0].into_iter().enumerate() {
                let half = (radius * radius - dy * dy).sqrt();
                let shade = 0.32 - i as f32 * 0.05;
                let band = LinearRgba::rgb(0.9 * shade, 0.4 * shade, 0.1 * shade);
                let y = center.y + dy;
                gizmos.line_2d(Vec2::new(center.x - half, y), Vec2::new(center.x + half, y), band);
            }
        }
        Backdrop::DataRain => {
            // Senkrechte Datenströme, die langsam herabrieseln.
            let t = time.elapsed_secs();
            for column in 0..34 {
                let x = -SPAN + 60.0 + column as f32 * 80.0;
                let speed = 60.0 + (column * 37 % 11) as f32 * 9.0;
                let height = 2.0 * HALF_HEIGHT;
                let y = HALF_HEIGHT - ((t * speed + column as f32 * 97.0) % height);
                for k in 0..3 {
                    let yk = y + k as f32 * 22.0;
                    let fade = 0.35 - k as f32 * 0.1;
                    gizmos.line_2d(Vec2::new(x, yk), Vec2::new(x, yk - 12.0), LinearRgba::rgb(0.1 * fade, 1.2 * fade, 0.3 * fade));
                }
            }
        }
    }

    // Leuchtendes Gitter als Boden und Decke, perspektivisch nach außen auseinanderlaufend.
    let grid = level.def().grid;
    for (edge, inner) in [(-HALF_HEIGHT, -HALF_HEIGHT + 40.0), (HALF_HEIGHT, HALF_HEIGHT - 40.0)] {
        gizmos.line_2d(Vec2::new(-SPAN, inner), Vec2::new(SPAN, inner), grid);
        let mut x = -SPAN - field.grid_offset;
        while x < SPAN {
            gizmos.line_2d(Vec2::new(x, inner), Vec2::new(x * 1.25, edge), grid);
            x += GRID_SPACING;
        }
    }
}
