//! Die Level mit Thema, Aussehen und Boss, und der Fortschritt durch das laufende Level. Der
//! Fortschritt läuft in Spielzeit, steht in der Zeitlupe also fast still. Am Ende wartet der
//! Boss.

use bevy::prelude::*;

use crate::enemy::Formation;
use crate::neon::{GOLD, GREEN, ICE, MAGENTA, ORANGE, SILVER, TEAL, VIOLET};
use crate::save::Mastery;
use crate::shipyard::Credits;
use crate::task::{BigTimesTable, BinaryNumbers, DecimalFractions, NegativeNumbers, ReducingFractions, RomanNumerals, FAST_SHARE, Outcome, SmallTimesTable, Squares, TaskGenerator, Topic};
use crate::{AppState, GamePhase, Gameplay};

/// Länge eines Levels bis zum Boss in Sekunden Spielzeit.
pub const LEVEL_LENGTH: f32 = 120.0;
/// Hier liegt der Checkpoint mitten im Level, als Anteil der Levellänge. Ein zweiter liegt
/// direkt vor dem Boss.
pub const CHECKPOINT_SHARE: f32 = 0.6;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LevelProgress>()
            .init_resource::<LevelStats>()
            .init_resource::<CurrentLevel>()
            .init_resource::<Campaign>()
            .add_message::<LevelStarted>()
            .add_message::<Rewind>()
            .add_message::<CheckpointReached>()
            .add_systems(Update, ((advance, reach_checkpoints).chain().in_set(Gameplay), begin_level, rewind));
    }
}

/// Was ein Level ausmacht: Rechenthema, Aussehen und Boss.
pub struct LevelDef {
    pub world: &'static str,
    pub topic: &'static str,
    /// Farbe des Gitters am oberen und unteren Rand.
    pub grid: LinearRgba,
    /// Was hinter den Sternen zu sehen ist.
    pub backdrop: Backdrop,
    pub boss: BossDef,
    pub make_topic: fn() -> Box<dyn TaskGenerator>,
    /// Aus diesen Formationen wird jede Welle gelost; was öfter dasteht, kommt öfter.
    pub formations: &'static [Formation],
}

pub struct BossDef {
    pub name: &'static str,
    pub color: LinearRgba,
    pub style: BossStyle,
}

/// Bauform des Bosses; jede hat ihren eigenen Rumpf.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BossStyle {
    /// Runde Festung
    Fortress,
    /// Würfel mit Gitterlinien
    Cube,
    /// Wirbelnde Sturmfestung; angeschlagen feuert sie zusätzlich Ringe aus Schüssen.
    Storm,
    /// Ein Chip mit Anschlussbeinchen
    Processor,
    /// Ein Legionärsschild mit Helmbusch
    Shield,
    /// Ein riesiger Eiskristall mit sechs verzweigten Armen
    Crystal,
    /// Ein Rad aus Tortenstücken mit breitem Außenring
    Wheel,
    /// Eine waagerecht geteilte Raute, oben und unten gespiegelt
    Mirror,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    /// Nur Sterne
    Stars,
    /// Ein riesiger, gestreifter Planet am unteren Rand
    GasGiant,
    /// Herabrieselnde Datenströme
    DataRain,
    /// Ein roter Planet mit Kratern
    RedPlanet,
    /// Ein Eismond am oberen Rand und quer durchs Bild die Nulllinie eines Zahlenstrahls
    IceMoon,
    /// Ein schräg stehender Riesenring mit Segmenten um einen kleinen Planeten
    RingWorld,
    /// Eine Spiegelachse quer durchs Bild, darüber eine Station und darunter ihr Spiegelbild
    MirrorStation,
}

fn small_times_table() -> Box<dyn TaskGenerator> {
    Box::new(SmallTimesTable::new())
}

fn squares() -> Box<dyn TaskGenerator> {
    Box::new(Squares::new())
}

fn big_times_table() -> Box<dyn TaskGenerator> {
    Box::new(BigTimesTable::new())
}

fn binary_numbers() -> Box<dyn TaskGenerator> {
    Box::new(BinaryNumbers::new())
}

fn roman_numerals() -> Box<dyn TaskGenerator> {
    Box::new(RomanNumerals::new())
}

fn negative_numbers() -> Box<dyn TaskGenerator> {
    Box::new(NegativeNumbers::new())
}

fn reducing_fractions() -> Box<dyn TaskGenerator> {
    Box::new(ReducingFractions::new())
}

fn decimal_fractions() -> Box<dyn TaskGenerator> {
    Box::new(DecimalFractions::new())
}

pub const LEVELS: [LevelDef; 8] = [
    LevelDef {
        world: "Neonnebel",
        topic: "Kleines 1×1",
        grid: LinearRgba::rgb(0.35, 0.08, 0.6),
        backdrop: Backdrop::Stars,
        boss: BossDef { name: "Der Wächter", color: MAGENTA, style: BossStyle::Fortress },
        make_topic: small_times_table,
        formations: &[Formation::Snake, Formation::Arrow, Formation::Pincer, Formation::Dive, Formation::Ring],
    },
    LevelDef {
        world: "Gitterstation",
        topic: "Quadratzahlen",
        grid: LinearRgba::rgb(0.05, 0.45, 0.4),
        backdrop: Backdrop::Stars,
        boss: BossDef { name: "Der Kubus", color: VIOLET, style: BossStyle::Cube },
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
    LevelDef {
        world: "Gasriese",
        topic: "Großes 1×1",
        grid: LinearRgba::rgb(0.6, 0.25, 0.04),
        backdrop: Backdrop::GasGiant,
        boss: BossDef { name: "Die Sturmfestung", color: ORANGE, style: BossStyle::Storm },
        make_topic: big_times_table,
        // In den Stürmen treiben Minen; Kreuzer feuern Fächer.
        formations: &[
            Formation::MineField,
            Formation::Cruisers,
            Formation::MineField,
            Formation::Snake,
            Formation::MineField,
            Formation::Dive,
            Formation::Seekers,
        ],
    },
    LevelDef {
        world: "Datenkern",
        topic: "Binärzahlen",
        grid: LinearRgba::rgb(0.05, 0.55, 0.15),
        backdrop: Backdrop::DataRain,
        boss: BossDef { name: "Der Prozessor", color: GREEN, style: BossStyle::Processor },
        make_topic: binary_numbers,
        // Bits, die nur als 1 verwundbar sind, und schnelle Datenströme.
        formations: &[
            Formation::Nibbles,
            Formation::Stream,
            Formation::Nibbles,
            Formation::Stream,
            Formation::Turrets,
            Formation::Splitters,
        ],
    },
    LevelDef {
        world: "Mars-Außenposten",
        topic: "Römische Zahlen",
        grid: LinearRgba::rgb(0.6, 0.12, 0.05),
        backdrop: Backdrop::RedPlanet,
        boss: BossDef { name: "Der Zenturio", color: GOLD, style: BossStyle::Shield },
        make_topic: roman_numerals,
        // Legionäre in Schildkröten-Formation, dazu Geschütze, Pfeile und Kreuzer.
        formations: &[
            Formation::Testudo,
            Formation::Testudo,
            Formation::Arrow,
            Formation::Turrets,
            Formation::Testudo,
            Formation::Pincer,
            Formation::Cruisers,
        ],
    },
    LevelDef {
        world: "Eismond",
        topic: "Negative Zahlen",
        grid: LinearRgba::rgb(0.12, 0.3, 0.65),
        backdrop: Backdrop::IceMoon,
        boss: BossDef { name: "Der Frostkristall", color: ICE, style: BossStyle::Crystal },
        make_topic: negative_numbers,
        // Frostlinge pendeln über die Nulllinie und sind nur oben, im Plus, verwundbar.
        formations: &[
            Formation::ZeroLine,
            Formation::ZeroLine,
            Formation::Arrow,
            Formation::MineField,
            Formation::ZeroLine,
            Formation::Dive,
            Formation::Seekers,
        ],
    },
    LevelDef {
        world: "Ringwelt",
        topic: "Brüche kürzen",
        grid: LinearRgba::rgb(0.05, 0.5, 0.3),
        backdrop: Backdrop::RingWorld,
        boss: BossDef { name: "Die Ringnabe", color: TEAL, style: BossStyle::Wheel },
        make_topic: reducing_fractions,
        // Räder, deren Nabe erst verwundbar ist, wenn alle Stücke ab sind; dazu Teiler.
        formations: &[
            Formation::Wheel,
            Formation::Wheel,
            Formation::Splitters,
            Formation::Snake,
            Formation::Wheel,
            Formation::Ring,
            Formation::Seekers,
        ],
    },
    LevelDef {
        world: "Spiegelstation",
        topic: "Brüche & Dezimalzahlen",
        grid: LinearRgba::rgb(0.35, 0.38, 0.55),
        backdrop: Backdrop::MirrorStation,
        boss: BossDef { name: "Der Spiegel", color: SILVER, style: BossStyle::Mirror },
        make_topic: decimal_fractions,
        // Zwillinge fliegen spiegelbildlich; wer einen trifft, trifft auch den anderen.
        formations: &[
            Formation::Twins,
            Formation::Twins,
            Formation::Pincer,
            Formation::Twins,
            Formation::Wheel,
            Formation::Arrow,
            Formation::Cruisers,
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
    /// Je Level drei Sterne als Bits: geschafft, 80 % richtig, Boss mit voller Superwaffe.
    pub stars: Vec<u8>,
}

impl Default for Campaign {
    fn default() -> Self {
        Self { unlocked: 1, beaten: 0, stars: vec![0; LEVELS.len()] }
    }
}

impl Campaign {
    pub fn stars_of(&self, level: usize) -> u8 {
        self.stars.get(level).copied().unwrap_or(0)
    }

    pub fn total_stars(&self) -> u32 {
        self.stars.iter().map(|mask| mask.count_ones()).sum()
    }
}

/// Drei Sterne als Zeichen, gefüllt für jeden erreichten: „★★☆“.
pub fn star_string(mask: u8) -> String {
    (0..3).map(|i| if mask & (1 << i) != 0 { '★' } else { '☆' }).collect()
}

/// Ein Level beginnt. Jedes Modul räumt darauf hin seinen Teil auf: Gegner, Boss, Aufgaben,
/// Ladungen. Waffen und Schild bleiben, wie sie sind.
#[derive(Message)]
pub struct LevelStarted(pub usize);

/// Das Schiff ist verloren: Das Level geht ab dem letzten Checkpoint weiter. Jedes Modul räumt
/// darauf hin das Feld; Statistik, Superwaffe und gesicherte Credits bleiben.
#[derive(Message)]
pub struct Rewind;

/// Ein Checkpoint ist erreicht; so viele Flug-Credits wurden dabei gesichert.
#[derive(Message)]
pub struct CheckpointReached {
    pub before_boss: bool,
    pub secured: u32,
}

#[derive(Resource, Default)]
pub struct LevelProgress {
    pub elapsed: f32,
    /// Spielzeit des zuletzt erreichten Checkpoints; 0 heißt Levelanfang.
    pub checkpoint: f32,
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
    *progress = LevelProgress::default();
    *stats = LevelStats::default();
    time.unpause();
    time.set_relative_speed(1.0);
    phase.set(GamePhase::Flying);
    app.set(AppState::Playing);
}

fn advance(time: Res<Time>, mut progress: ResMut<LevelProgress>) {
    progress.elapsed += time.delta_secs();
}

/// Checkpoints mitten im Level und vor dem Boss: Die Flug-Credits wandern aufs Konto.
fn reach_checkpoints(
    mut progress: ResMut<LevelProgress>,
    mut credits: ResMut<Credits>,
    mut reached: MessageWriter<CheckpointReached>,
) {
    for at in [CHECKPOINT_SHARE * LEVEL_LENGTH, LEVEL_LENGTH] {
        if progress.elapsed >= at && progress.checkpoint < at {
            progress.checkpoint = at;
            let secured = credits.secure();
            info!("Checkpoint erreicht, {secured} Credits gesichert");
            reached.write(CheckpointReached { before_boss: at >= LEVEL_LENGTH, secured });
        }
    }
}

/// Zurück zum letzten Checkpoint.
fn rewind(mut rewinds: MessageReader<Rewind>, mut progress: ResMut<LevelProgress>) {
    if rewinds.read().count() > 0 {
        progress.elapsed = progress.checkpoint;
    }
}
