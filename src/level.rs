//! Fortschritt durch das Level. Er läuft in Spielzeit, steht in der Zeitlupe also fast still.
//! Am Ende wartet der Boss; den gibt es noch nicht, bis dahin geht der Flug einfach weiter.

use bevy::prelude::*;

/// Länge eines Levels bis zum Boss in Sekunden Spielzeit.
pub const LEVEL_LENGTH: f32 = 240.0;
/// Hier liegt der Checkpoint, als Anteil der Levellänge.
pub const CHECKPOINT_SHARE: f32 = 0.6;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelProgress>().add_systems(Update, advance);
    }
}

#[derive(Resource, Default)]
pub struct LevelProgress {
    pub elapsed: f32,
}

impl LevelProgress {
    pub fn share(&self) -> f32 {
        (self.elapsed / LEVEL_LENGTH).min(1.0)
    }

    pub fn boss_ahead(&self) -> bool {
        self.elapsed >= LEVEL_LENGTH
    }
}

fn advance(time: Res<Time>, mut progress: ResMut<LevelProgress>) {
    progress.elapsed += time.delta_secs();
}
