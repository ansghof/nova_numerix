//! Funken, Trümmer und Druckwellen als schlanke CPU-Simulation ohne eigene Entities.
//! Läuft in Spielzeit, also in der Zeitlupe ebenfalls langsamer.

use bevy::prelude::*;

use crate::neon::{Shape, WHITE, scaled};
use crate::rng::Rng;

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Particles>().add_systems(Update, (update, draw).chain());
    }
}

enum Kind {
    /// Ein Funke, der als Strich in Flugrichtung gezeichnet wird.
    Spark { length: f32 },
    /// Ein Stück Umriss einer zerstörten Figur, das sich dreht.
    Debris { half: Vec2, spin: f32 },
    /// Ein wachsender Ring.
    Ring { speed: f32 },
}

struct Particle {
    kind: Kind,
    pos: Vec2,
    vel: Vec2,
    angle: f32,
    age: f32,
    life: f32,
    color: LinearRgba,
}

#[derive(Resource, Default)]
pub struct Particles(Vec<Particle>);

impl Particles {
    pub fn sparks(&mut self, rng: &mut Rng, at: Vec2, color: LinearRgba, count: usize, speed: f32) {
        for _ in 0..count {
            let vel = rng.direction() * speed * rng.range(0.2, 1.0);
            self.0.push(Particle {
                kind: Kind::Spark { length: rng.range(4.0, 12.0) },
                pos: at,
                vel,
                angle: 0.0,
                age: 0.0,
                life: rng.range(0.3, 0.8),
                color,
            });
        }
    }

    /// Feuerstrahl hinter dem Schiff: kurze Funken in eine Richtung.
    pub fn exhaust(&mut self, rng: &mut Rng, at: Vec2, dir: Vec2, color: LinearRgba) {
        self.0.push(Particle {
            kind: Kind::Spark { length: 6.0 },
            pos: at + Vec2::new(0.0, rng.range(-3.0, 3.0)),
            vel: dir * rng.range(150.0, 260.0) + Vec2::new(0.0, rng.range(-20.0, 20.0)),
            angle: 0.0,
            age: 0.0,
            life: rng.range(0.1, 0.22),
            color,
        });
    }

    pub fn ring(&mut self, at: Vec2, color: LinearRgba, speed: f32, life: f32) {
        self.0.push(Particle { kind: Kind::Ring { speed }, pos: at, vel: Vec2::ZERO, angle: 0.0, age: 0.0, life, color });
    }

    /// Eine Explosion: Die Figur zerfällt in ihre Linien, dazu Funken, ein Blitz und ein Ring.
    pub fn explode(&mut self, rng: &mut Rng, shape: &Shape, transform: &Transform, size: f32) {
        let center = transform.translation.truncate();
        for (a, b) in shape.segments() {
            let a = transform.transform_point(a.extend(0.0)).truncate();
            let b = transform.transform_point(b.extend(0.0)).truncate();
            let mid = (a + b) / 2.0;
            let outward = (mid - center).normalize_or(rng.direction());
            self.0.push(Particle {
                kind: Kind::Debris { half: (b - a) / 2.0, spin: rng.range(-8.0, 8.0) },
                pos: mid,
                vel: outward * rng.range(60.0, 220.0) * size + rng.direction() * 40.0,
                angle: 0.0,
                age: 0.0,
                life: rng.range(0.6, 1.2),
                color: shape.color,
            });
        }
        self.sparks(rng, center, shape.color, (24.0 * size) as usize, 380.0 * size);
        self.sparks(rng, center, WHITE, (10.0 * size) as usize, 200.0 * size);
        self.ring(center, scaled(shape.color, 0.8), 260.0 * size, 0.35);
    }
}

fn update(time: Res<Time>, mut particles: ResMut<Particles>) {
    let dt = time.delta_secs();
    particles.0.retain_mut(|p| {
        p.age += dt;
        p.pos += p.vel * dt;
        // Luftwiderstand gibt es im All nicht, aber so sehen Explosionen besser aus.
        p.vel *= 1.0 - (2.5 * dt).min(1.0);
        if let Kind::Debris { spin, .. } = p.kind {
            p.angle += spin * dt;
        }
        p.age < p.life
    });
}

fn draw(mut gizmos: Gizmos, particles: Res<Particles>) {
    for p in &particles.0 {
        let fade = 1.0 - p.age / p.life;
        let color = scaled(p.color, fade);
        match p.kind {
            Kind::Spark { length } => {
                let tail = p.vel.normalize_or_zero() * length * fade.max(0.3);
                gizmos.line_2d(p.pos - tail, p.pos, color);
            }
            Kind::Debris { half, .. } => {
                let half = Vec2::from_angle(p.angle).rotate(half);
                gizmos.line_2d(p.pos - half, p.pos + half, color);
            }
            Kind::Ring { speed } => {
                gizmos.circle_2d(p.pos, speed * p.age, color).resolution(48);
            }
        }
    }
}
