//! Neon-Vektorgrafik: Figuren bestehen aus leuchtenden Linienzügen, die jedes Bild neu als
//! Gizmos gezeichnet werden. Farben über 1.0 lässt das Bloom der Kamera glühen.

use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub const CYAN: LinearRgba = LinearRgba::rgb(0.4, 2.6, 3.4);
pub const WHITE: LinearRgba = LinearRgba::rgb(3.0, 3.0, 3.2);
pub const MAGENTA: LinearRgba = LinearRgba::rgb(3.2, 0.3, 2.4);
pub const RED: LinearRgba = LinearRgba::rgb(3.6, 0.4, 0.5);
pub const ORANGE: LinearRgba = LinearRgba::rgb(4.0, 1.3, 0.2);
pub const GOLD: LinearRgba = LinearRgba::rgb(3.8, 2.6, 0.4);
pub const GREEN: LinearRgba = LinearRgba::rgb(0.6, 3.6, 1.0);

pub struct NeonPlugin;

impl Plugin for NeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, configure_lines)
            .add_systems(PostUpdate, draw_shapes.after(TransformSystems::Propagate));
    }
}

/// Eine Figur aus Linienzügen in lokalen Koordinaten.
#[derive(Component, Clone)]
pub struct Shape {
    pub strokes: Vec<Stroke>,
    pub color: LinearRgba,
    pub visible: bool,
}

#[derive(Clone)]
pub struct Stroke {
    pub points: Vec<Vec2>,
    pub closed: bool,
}

impl Shape {
    pub fn new(color: LinearRgba) -> Self {
        Self { strokes: Vec::new(), color, visible: true }
    }

    /// Ein geschlossener Umriss.
    pub fn closed(mut self, points: &[Vec2]) -> Self {
        self.strokes.push(Stroke { points: points.to_vec(), closed: true });
        self
    }

    /// Ein offener Linienzug.
    pub fn open(mut self, points: &[Vec2]) -> Self {
        self.strokes.push(Stroke { points: points.to_vec(), closed: false });
        self
    }

    /// Alle Einzelstrecken der Figur, etwa um sie in Trümmer zu zerlegen.
    pub fn segments(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        self.strokes.iter().flat_map(|stroke| {
            let n = stroke.points.len();
            let count = if stroke.closed { n } else { n.saturating_sub(1) };
            (0..count).map(move |i| (stroke.points[i], stroke.points[(i + 1) % n]))
        })
    }
}

/// Eckpunkte eines regelmäßigen Vielecks.
pub fn polygon(sides: usize, radius: f32, rotation: f32) -> Vec<Vec2> {
    (0..sides)
        .map(|i| Vec2::from_angle(rotation + i as f32 / sides as f32 * std::f32::consts::TAU) * radius)
        .collect()
}

/// Dieselbe Leuchtfarbe, abgedunkelt oder aufgehellt.
pub fn scaled(color: LinearRgba, factor: f32) -> LinearRgba {
    LinearRgba::new(color.red * factor, color.green * factor, color.blue * factor, color.alpha)
}

fn configure_lines(mut store: ResMut<GizmoConfigStore>) {
    let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 2.5;
    config.line.joints = GizmoLineJoint::Round(4);
}

fn draw_shapes(mut gizmos: Gizmos, shapes: Query<(&Shape, &GlobalTransform)>) {
    for (shape, transform) in &shapes {
        if !shape.visible {
            continue;
        }
        for stroke in &shape.strokes {
            let world = stroke
                .points
                .iter()
                .map(|p| transform.transform_point(p.extend(0.0)).truncate());
            if stroke.closed {
                let first = world.clone().next();
                gizmos.linestrip_2d(world.chain(first), shape.color);
            } else {
                gizmos.linestrip_2d(world, shape.color);
            }
        }
    }
}
