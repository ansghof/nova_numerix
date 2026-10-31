//! Schwierigkeitsgrade Kadett, Pilot und Ass. Sie ändern, wie viel Zeit eine Aufgabe hat, wie
//! viel Schaden Treffer machen und wie dicht die Wellen kommen. Gewählt wird beim neuen Spiel,
//! ändern lässt er sich jederzeit in der Werft. Sterne gibt es auf jedem Grad gleich.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::task::Topic;

pub struct DifficultyPlugin;

impl Plugin for DifficultyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Difficulty>().add_systems(Update, apply_time_factor);
    }
}

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Difficulty {
    Kadett,
    #[default]
    Pilot,
    Ass,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Kadett, Difficulty::Pilot, Difficulty::Ass];

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Kadett => "Kadett",
            Difficulty::Pilot => "Pilot",
            Difficulty::Ass => "Ass",
        }
    }

    /// In einem Satz, was der Grad bedeutet.
    pub fn describe(self) -> &'static str {
        match self {
            Difficulty::Kadett => "mehr Zeit zum Rechnen, Treffer schaden weniger, weniger Gegner",
            Difficulty::Pilot => "so wie gedacht",
            Difficulty::Ass => "weniger Zeit, Treffer schaden mehr, dichtere Wellen",
        }
    }

    /// Der nächste Grad, im Kreis; `back` geht rückwärts.
    pub fn cycle(self, back: bool) -> Self {
        let i = Self::ALL.iter().position(|d| *d == self).unwrap_or(1);
        let n = Self::ALL.len();
        Self::ALL[if back { (i + n - 1) % n } else { (i + 1) % n }]
    }

    /// Faktor auf das Zeitlimit jeder Aufgabe.
    pub fn time_factor(self) -> f32 {
        match self {
            Difficulty::Kadett => 1.4,
            Difficulty::Pilot => 1.0,
            Difficulty::Ass => 0.8,
        }
    }

    /// Faktor auf den Schaden durch Schüsse und Zusammenstöße.
    pub fn damage_factor(self) -> f32 {
        match self {
            Difficulty::Kadett => 0.6,
            Difficulty::Pilot => 1.0,
            Difficulty::Ass => 1.3,
        }
    }

    /// Faktor auf den Abstand zwischen zwei Wellen.
    pub fn wave_gap_factor(self) -> f32 {
        match self {
            Difficulty::Kadett => 1.3,
            Difficulty::Pilot => 1.0,
            Difficulty::Ass => 0.8,
        }
    }
}

fn apply_time_factor(difficulty: Res<Difficulty>, mut topic: ResMut<Topic>) {
    if difficulty.is_changed() {
        topic.1 = difficulty.time_factor();
    }
}
