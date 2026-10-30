//! Die Level mit Thema, Aussehen und Boss, und der Fortschritt durch das laufende Level. Der
//! Fortschritt läuft in Spielzeit, steht in der Zeitlupe also fast still. Am Ende wartet der
//! Boss.

use bevy::prelude::*;

use crate::enemy::Formation;
use crate::neon::{MAGENTA, VIOLET};
use crate::save::Mastery;
use crate::task::{FAST_SHARE, Outcome, SmallTimesTable, Squares, TaskGenerator, Topic};
use crate::{AppState, GamePhase, Gameplay};

/// Länge eines Levels bis zum Boss in Sekunden Spielzeit.
pub const LEVEL_LENGTH: f32 = 120.0;
/// Hier liegt der Checkpoint, als Anteil der Levellänge.
pub const CHECKPOINT_SHARE: f32 = 0.6;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelProgress>()
            .init_resource::<LevelStats>()
            .init_resource::<CurrentLevel>()
            .init_resource::<Campaign>()
            .add_message::<LevelStarted>()
            .add_systems(Update, (advance.in_set(Gameplay), begin_level));
    }
}

/// Was ein Level ausmacht: Rechenthema, Aussehen und Boss.
pub struct LevelDef {
    pub world: &'static str,
    pub topic: &'static str,
    /// Farbe des Gitters am oberen und unteren Rand.
    pub grid: LinearRgba,
    pub boss: BossDef,
    pub make_topic: fn() -> Box<dyn TaskGenerator>,
    /// Aus diesen Formationen wird jede Welle gelost; was öfter dasteht, kommt öfter.
    pub formations: &'static [Formation],
}

pub struct BossDef {
    pub name: &'static str,
    pub color: LinearRgba,
    /// Würfelförmiger Rumpf statt der runden Festung.
    pub cube: bool,
}

fn small_times_table() -> Box<dyn TaskGenerator> {
    Box::new(SmallTimesTable::new())
}

fn squares() -> Box<dyn TaskGenerator> {
    Box::new(Squares::new())
}

pub const LEVELS: [LevelDef; 2] = [
    LevelDef {
        world: "Neonnebel",
        topic: "Kleines 1×1",
        grid: LinearRgba::rgb(0.35, 0.08, 0.6),
        boss: BossDef { name: "Der Wächter", color: MAGENTA, cube: false },
        make_topic: small_times_table,
        formations: &[Formation::Snake, Formation::Arrow, Formation::Pincer, Formation::Dive, Formation::Ring],
    },
    LevelDef {
        world: "Gitterstation",
        topic: "Quadratzahlen",
        grid: LinearRgba::rgb(0.05, 0.45, 0.4),
        boss: BossDef { name: "Der Kubus", color: VIOLET, cube: true },
        make_topic: squares,
        // Die Station hat eigene Verteidiger; von den alten Bekannten kommen nur noch wenige.
        formations: &[
            Formation::Turrets,
            Formation::Splitters,
            Formation::Seekers,
            Formation::Turrets,
            Formation::Splitters,
            Formation::Seekers,
            Formation::Arrow,
            Formation::Ring,
        ],
    },
];

/// Das Level, das gerade geflogen wird.
#[derive(Resource, Default)]
pub struct CurrentLevel(pub usize);

impl CurrentLevel {
    pub fn def(&self) -> &'static LevelDef {
        &LEVELS[self.0]
    }
}

/// Wie viele Level freigeschaltet und wie viele Bosse schon besiegt sind. Mit jedem besiegten
/// Boss bekommt Quibbo neue Ware.
#[derive(Resource)]
pub struct Campaign {
    pub unlocked: usize,
    pub beaten: usize,
}

impl Default for Campaign {
    fn default() -> Self {
        Self { unlocked: 1, beaten: 0 }
    }
}

/// Ein Level beginnt. Jedes Modul räumt darauf hin seinen Teil auf: Gegner, Boss, Aufgaben,
/// Ladungen. Waffen und Schild bleiben, wie sie sind.
#[derive(Message)]
pub struct LevelStarted(pub usize);

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

pub fn begin_level(
    mut started: MessageReader<LevelStarted>,
    mut current: ResMut<CurrentLevel>,
    mut topic: ResMut<Topic>,
    mut mastery: ResMut<Mastery>,
    mut progress: ResMut<LevelProgress>,
    mut stats: ResMut<LevelStats>,
    mut time: ResMut<Time<Virtual>>,
    mut phase: ResMut<NextState<GamePhase>>,
    mut app: ResMut<NextState<AppState>>,
) {
    let Some(LevelStarted(index)) = started.read().last() else {
        return;
    };
    info!("Level {} beginnt", index + 1);
    // Lernstand des bisherigen Themas aufheben, den des neuen wiederherstellen.
    mastery.0.insert(current.def().topic.to_string(), topic.0.weights());
    current.0 = *index;
    let def = current.def();
    topic.0 = (def.make_topic)();
    if let Some(weights) = mastery.0.get(def.topic) {
        topic.0.set_weights(weights);
    }
    progress.elapsed = 0.0;
    *stats = LevelStats::default();
    time.unpause();
    time.set_relative_speed(1.0);
    phase.set(GamePhase::Flying);
    app.set(AppState::Playing);
}

fn advance(time: Res<Time>, mut progress: ResMut<LevelProgress>) {
    progress.elapsed += time.delta_secs();
}
