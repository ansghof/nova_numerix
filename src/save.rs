//! Spielstände in fünf Slots. Gespeichert wird automatisch in der Werft: beim Ankommen, nach
//! jedem Kauf oder Training und beim Abflug. Ein Slot trägt den Namen des Schiffs und den
//! Zeitpunkt der letzten Speicherung.
//!
//! Wo die Daten liegen, entscheidet [`SaveStorage`]: auf dem PC Dateien im Benutzerordner,
//! im Browser später der localStorage.

use std::collections::HashMap;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::hud::Score;
use crate::level::{Campaign, CurrentLevel, LEVELS, LevelStarted, begin_level};
use crate::player::{Loadout, MAX_HEALTH, Player};
use crate::difficulty::Difficulty;
use crate::ship::{Hull, Ship};
use crate::shipyard::Credits;
use crate::superweapon::SuperWeapon;
use crate::task::{SlowChips, Topic};

pub const SLOTS: usize = 5;
/// Ändert sich der Aufbau des Spielstands, wird die Zahl erhöht; ältere Stände gelten dann
/// als veraltet.
const VERSION: u32 = 1;
pub const DEFAULT_SHIP_NAME: &str = "Sternfalke";
pub const MAX_NAME_LENGTH: usize = 20;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Saves { storage: Box::new(FileStorage::new()), active: None })
            .init_resource::<ShipName>()
            .init_resource::<Mastery>()
            .add_systems(OnEnter(AppState::Shipyard), save_now)
            .add_systems(
                Update,
                (
                    save_now.run_if(in_state(AppState::Shipyard)).run_if(resource_changed::<Credits>),
                    save_on_departure.after(begin_level),
                ),
            );
    }
}

/// Wo Spielstände liegen.
pub trait SaveStorage: Send + Sync + 'static {
    fn read(&self, slot: usize) -> Option<String>;
    fn write(&self, slot: usize, data: &str) -> Result<(), String>;
}

/// Eine RON-Datei pro Slot im Benutzerordner; Testläufe schreiben nach `target`.
struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    fn new() -> Self {
        let dir = if cfg!(debug_assertions) && std::env::var("NOVA_TEST").is_ok() {
            PathBuf::from("target").join("test-saves")
        } else {
            match std::env::var_os("APPDATA") {
                Some(dir) => PathBuf::from(dir).join("NovaNumerix"),
                None => PathBuf::from("spielstaende"),
            }
        };
        Self { dir }
    }

    fn path(&self, slot: usize) -> PathBuf {
        self.dir.join(format!("slot{}.ron", slot + 1))
    }
}

impl SaveStorage for FileStorage {
    fn read(&self, slot: usize) -> Option<String> {
        std::fs::read_to_string(self.path(slot)).ok()
    }

    fn write(&self, slot: usize, data: &str) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        std::fs::write(self.path(slot), data).map_err(|e| e.to_string())
    }
}

#[derive(Resource)]
pub struct Saves {
    storage: Box<dyn SaveStorage>,
    /// In welchen Slot das laufende Spiel gespeichert wird.
    pub active: Option<usize>,
}

impl Saves {
    pub fn load(&self, slot: usize) -> Option<SaveData> {
        let data: SaveData = ron::from_str(&self.storage.read(slot)?).ok()?;
        (data.version == VERSION).then_some(data)
    }

    /// Kurzbeschreibung jedes Slots für die Auswahl; `None` bei einem freien Slot.
    pub fn overview(&self) -> [Option<(String, String)>; SLOTS] {
        std::array::from_fn(|slot| {
            let raw = self.storage.read(slot)?;
            Some(match ron::from_str::<SaveData>(&raw) {
                Ok(data) if data.version == VERSION => {
                    let stars: u32 = data.stars.iter().map(|mask| mask.count_ones()).sum();
                    let detail = format!(
                        "{} von {} Leveln frei · ★ {stars}/{} · {} · {} Credits",
                        data.unlocked,
                        LEVELS.len(),
                        LEVELS.len() * 3,
                        data.difficulty.name(),
                        data.credits
                    );
                    (format!("{} · {}", data.ship_name, data.saved_at), detail)
                }
                _ => ("Unlesbarer oder veralteter Spielstand".into(), "wird beim Speichern überschrieben".into()),
            })
        })
    }
}

/// Der Name des Schiffs, auch im HUD und in der Slot-Liste.
#[derive(Resource)]
pub struct ShipName(pub String);

impl Default for ShipName {
    fn default() -> Self {
        Self(DEFAULT_SHIP_NAME.into())
    }
}

/// Lernstand je Rechenthema: die Gewichte der einzelnen Fakten, damit schwache Aufgaben auch
/// nach einem Neustart häufiger kommen.
#[derive(Resource, Default)]
pub struct Mastery(pub HashMap<String, Vec<f32>>);

#[derive(Serialize, Deserialize)]
pub struct SaveData {
    version: u32,
    ship_name: String,
    saved_at: String,
    credits: u32,
    laser: u8,
    missiles: u8,
    #[serde(default)]
    side: u8,
    #[serde(default)]
    drones: u8,
    #[serde(default)]
    hull: u8,
    #[serde(default)]
    armor: u8,
    health: f32,
    bought_charges: u8,
    slow_chips: u32,
    unlocked: usize,
    /// Fehlt in älteren Ständen; dann zählt jedes freigeschaltete Level als besiegt.
    #[serde(default)]
    beaten: usize,
    #[serde(default)]
    stars: Vec<u8>,
    #[serde(default)]
    difficulty: Difficulty,
    level: usize,
    score: u32,
    mastery: Vec<(String, Vec<f32>)>,
}

impl SaveData {
    fn new_game(ship_name: String, difficulty: Difficulty) -> Self {
        let loadout = Loadout::default();
        Self {
            version: VERSION,
            ship_name,
            saved_at: String::new(),
            credits: 0,
            laser: loadout.laser,
            missiles: loadout.missiles,
            side: loadout.side,
            drones: loadout.drones,
            hull: 0,
            armor: 0,
            health: MAX_HEALTH,
            bought_charges: 0,
            slow_chips: 0,
            unlocked: 1,
            beaten: 0,
            stars: Vec::new(),
            difficulty,
            level: 0,
            score: 0,
            mastery: Vec::new(),
        }
    }
}

/// Alles, was ein Spielstand umfasst, zum Lesen und Schreiben.
#[derive(bevy::ecs::system::SystemParam)]
pub struct GameState<'w, 's> {
    name: ResMut<'w, ShipName>,
    credits: ResMut<'w, Credits>,
    loadout: ResMut<'w, Loadout>,
    ship: ResMut<'w, Ship>,
    difficulty: ResMut<'w, Difficulty>,
    weapon: ResMut<'w, SuperWeapon>,
    chips: ResMut<'w, SlowChips>,
    campaign: ResMut<'w, Campaign>,
    current: ResMut<'w, CurrentLevel>,
    score: ResMut<'w, Score>,
    mastery: ResMut<'w, Mastery>,
    topic: ResMut<'w, Topic>,
    player: Single<'w, 's, &'static mut Player>,
}

impl GameState<'_, '_> {
    fn snapshot(&mut self) -> SaveData {
        // Den Lernstand des laufenden Themas vorher einsammeln.
        let topic = self.current.def().topic.to_string();
        self.mastery.0.insert(topic, self.topic.0.weights());
        let mut mastery: Vec<_> = self.mastery.0.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        mastery.sort_by(|a, b| a.0.cmp(&b.0));
        SaveData {
            version: VERSION,
            ship_name: self.name.0.clone(),
            saved_at: chrono::Local::now().format("%d.%m.%Y %H:%M").to_string(),
            credits: self.credits.bank,
            laser: self.loadout.laser,
            missiles: self.loadout.missiles,
            side: self.loadout.side,
            drones: self.loadout.drones,
            hull: self.ship.hull.index(),
            armor: self.ship.armor,
            health: self.player.health,
            bought_charges: self.weapon.bought,
            slow_chips: self.chips.0,
            unlocked: self.campaign.unlocked,
            beaten: self.campaign.beaten,
            stars: self.campaign.stars.clone(),
            difficulty: *self.difficulty,
            level: self.current.0,
            score: self.score.0,
            mastery,
        }
    }

    fn apply(&mut self, data: SaveData) {
        self.name.0 = data.ship_name;
        *self.credits = Credits { bank: data.credits, flight: 0, secured: 0 };
        *self.loadout = Loadout { laser: data.laser, missiles: data.missiles, side: data.side, drones: data.drones };
        *self.ship = Ship { hull: Hull::from_index(data.hull), armor: data.armor.min(crate::ship::MAX_ARMOR) };
        // Die Schildgrenze passt sich gleich an den Rumpf an; bis dahin nicht kappen.
        self.player.max_health = self.ship.hull.max_health();
        self.player.health = data.health;
        self.player.invulnerable = 0.0;
        self.weapon.bought = data.bought_charges;
        self.chips.0 = data.slow_chips;
        self.campaign.unlocked = data.unlocked.clamp(1, LEVELS.len());
        self.campaign.beaten = data.beaten.max(self.campaign.unlocked - 1).min(LEVELS.len());
        let mut stars = data.stars;
        stars.resize(LEVELS.len(), 0);
        self.campaign.stars = stars;
        *self.difficulty = data.difficulty;
        self.current.0 = data.level.min(LEVELS.len() - 1);
        self.score.0 = data.score;
        self.mastery.0 = data.mastery.into_iter().collect();
        let def = self.current.def();
        self.topic.0 = (def.make_topic)();
        if let Some(weights) = self.mastery.0.get(def.topic) {
            self.topic.0.set_weights(weights);
        }
    }
}

fn save_now(saves: Res<Saves>, mut state: GameState) {
    let Some(slot) = saves.active else {
        return;
    };
    let data = state.snapshot();
    let text = match ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::default().compact_arrays(true)) {
        Ok(text) => text,
        Err(error) => {
            warn!("Spielstand nicht erzeugt: {error}");
            return;
        }
    };
    match saves.storage.write(slot, &text) {
        Ok(()) => info!("Gespeichert in Slot {}", slot + 1),
        Err(error) => warn!("Spielstand nicht gespeichert: {error}"),
    }
}

fn save_on_departure(mut started: MessageReader<LevelStarted>, saves: Res<Saves>, state: GameState) {
    if started.read().last().is_some() {
        save_now(saves, state);
    }
}

/// Ein neues Spiel im gewählten Slot: frisches Schiff, erstes Level.
pub fn new_game(
    input: In<(usize, String, Difficulty)>,
    mut saves: ResMut<Saves>,
    mut state: GameState,
    mut started: MessageWriter<LevelStarted>,
) {
    let (slot, name, difficulty) = input.0;
    state.apply(SaveData::new_game(name, difficulty));
    saves.active = Some(slot);
    started.write(LevelStarted(0));
}

/// Einen Spielstand laden; es geht in der Werft weiter.
pub fn load_game(slot: In<usize>, mut saves: ResMut<Saves>, mut state: GameState, mut next: ResMut<NextState<AppState>>) {
    let Some(data) = saves.load(*slot) else {
        warn!("Slot {} lässt sich nicht laden", *slot + 1);
        return;
    };
    state.apply(data);
    saves.active = Some(*slot);
    next.set(AppState::Shipyard);
}
