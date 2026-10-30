//! Fortschritt durch das Level. Er läuft in Spielzeit, steht in der Zeitlupe also fast still.
//! Am Ende wartet der Boss.

use bevy::prelude::*;

use crate::Gameplay;
use crate::task::{FAST_SHARE, Outcome};

/// Länge eines Levels bis zum Boss in Sekunden Spielzeit.
pub const LEVEL_LENGTH: f32 = 120.0;
/// Hier liegt der Checkpoint, als Anteil der Levellänge.
pub const CHECKPOINT_SHARE: f32 = 0.6;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelProgress>().init_resource::<LevelStats>().add_systems(Update, advance.in_set(Gameplay));
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

/// Wie die Aufgaben in diesem Level ausgegangen sind, für die Auswertung am Ende.
#[derive(Resource, Default)]
pub struct LevelStats {
    pub correct: u32,
    pub fast: u32,
    pub timed_out: u32,
    pub wrong: u32,
}

impl LevelStats {
    pub fn count(&mut self, outcome: &Outcome) {
        match outcome {
            Outcome::Correct { remaining_share } => {
                self.correct += 1;
                if *remaining_share >= FAST_SHARE {
                    self.fast += 1;
                }
            }
            Outcome::TimedOut => self.timed_out += 1,
            Outcome::Wrong => self.wrong += 1,
        }
    }
}

fn advance(time: Res<Time>, mut progress: ResMut<LevelProgress>) {
    progress.elapsed += time.delta_secs();
}
