//! Die Werft zwischen zwei Flügen. Von der Übersicht geht es ins Rechentraining, das Credits
//! bringt, in den Laden von Quibbo, dem Rechenkrämer, und zum Abflug ins nächste Level.
//!
//! Das Training zahlt pro Besuch die ersten 25 richtigen Aufgaben voll aus, danach immer
//! weniger; nach jedem geflogenen Level beginnt die Staffel von vorn. So reicht es nach einem
//! Level für ein, zwei Käufe, aber nicht dafür, sofort übermächtig zu werden.
//!
//! Bedienung nur mit der Tastatur: W/S wählen, Enter bestätigen, Esc zurück.

use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::AppState;
use crate::level::{Campaign, CurrentLevel, LEVELS, LevelStarted};
use crate::player::{Loadout, MAX_DRONES, MAX_HEALTH, MAX_LEVEL, MAX_SIDE, Player};
use crate::quibbo::QuibboArt;
use crate::rng::Rng;
use crate::superweapon::SuperWeapon;
use crate::task::{FAST_SHARE, Outcome, SlowChips, Task, Topic, read_digit};

const BACKGROUND_IMAGE: &str = "menu.png";
/// Credits pro richtiger Trainingsaufgabe, dazu ein Tempo-Bonus.
const TRAINING_BASE: f32 = 12.0;
const TRAINING_SPEED: f32 = 8.0;
/// So viele richtige Aufgaben pro Besuch zahlen voll aus.
const FULL_TASKS: u32 = 25;
/// Nach einer falschen Antwort: kurz durchatmen, bevor die nächste kommt.
const WRONG_PAUSE: f32 = 3.0;
const TIMEOUT_PAUSE: f32 = 1.5;
const CORRECT_PAUSE: f32 = 0.6;
/// So lange steht Quibbos Antwort auf eine Verhandlung da.
const HAGGLE_RESULT_TIME: f32 = 1.8;
const REPAIR_STEP: f32 = 25.0;
const REPAIR_PER_POINT: u32 = 10;
const LASER_PRICE_PER_LEVEL: u32 = 400;
const MISSILE_LAUNCHER_PRICE: u32 = 600;
const SUPER_PRICE: u32 = 350;
const MAX_BOUGHT_CHARGES: u8 = 2;
const CHIP_PRICE: u32 = 200;
const CHIPS_PER_PACK: u32 = 3;

const TEXT: Color = Color::srgb(0.92, 0.96, 1.0);
const DIM: Color = Color::srgb(0.55, 0.62, 0.75);
const GOOD: Color = Color::srgb(0.45, 1.0, 0.6);
const OKAY: Color = Color::srgb(1.0, 0.85, 0.35);
const BAD: Color = Color::srgb(1.0, 0.4, 0.4);
const FRAME: Color = Color::srgb(0.3, 0.85, 1.0);
const PANEL: Color = Color::srgba(0.02, 0.04, 0.12, 0.9);
const ROW: Color = Color::srgba(0.05, 0.08, 0.2, 0.8);
const ROW_SELECTED: Color = Color::srgba(0.1, 0.25, 0.45, 0.95);
const BAR_BACK: Color = Color::srgb(0.08, 0.1, 0.2);
/// So lange bleibt Quibbo erfreut oder beleidigt, bevor er wieder neutral schaut.
const MOOD_TIME: f32 = 3.0;
const BLINK_EVERY: f32 = 3.2;
const BLINK_TIME: f32 = 0.15;

pub struct ShipyardPlugin;

impl Plugin for ShipyardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Credits>()
            .init_resource::<Training>()
            .add_systems(OnEnter(AppState::Shipyard), enter)
            .add_systems(OnExit(AppState::Shipyard), leave)
            .add_systems(Update, (update, update_bar, render, animate_quibbo).chain().run_if(in_state(AppState::Shipyard)))
            .add_systems(Update, new_visit_after_flight);
    }
}

/// Credits auf dem Konto und die im laufenden Flug gesammelten. Die aus dem Flug kommen erst
/// nach einem Sieg aufs Konto und gehen verloren, wenn das Schiff zerstört wird.
#[derive(Resource, Default)]
pub struct Credits {
    pub bank: u32,
    pub flight: u32,
}

/// Wie viele Trainingsaufgaben seit dem letzten Flug richtig gelöst wurden.
#[derive(Resource, Default)]
struct Training {
    solved: u32,
}

impl Training {
    fn factor(&self) -> f32 {
        match self.solved {
            0..25 => 1.0,
            25..40 => 0.5,
            40..60 => 0.25,
            _ => 0.1,
        }
    }
}

/// Eine Rechenaufgabe in der Werft, mit Eingabe und Zeitbalken.
struct Run {
    task: Task,
    input: String,
    elapsed: f32,
}

impl Run {
    fn new(task: Task) -> Self {
        Self { task, input: String::new(), elapsed: 0.0 }
    }

    fn share(&self) -> f32 {
        (1.0 - self.elapsed / self.task.time_limit).clamp(0.0, 1.0)
    }

    /// Nimmt Ziffern, Rücktaste und Enter entgegen; gibt das Ergebnis zurück, sobald es feststeht.
    fn feed(&mut self, keys: &ButtonInput<KeyCode>, dt: f32) -> Option<Outcome> {
        self.elapsed += dt;
        if let Some(digit) = read_digit(keys)
            && self.input.len() < 4
        {
            self.input.push(digit);
        }
        if keys.just_pressed(KeyCode::Backspace) {
            self.input.pop();
        }
        let submitted = (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && !self.input.is_empty();
        if submitted {
            return Some(if self.input.parse::<u32>() == Ok(self.task.answer) {
                Outcome::Correct { remaining_share: self.share() }
            } else {
                Outcome::Wrong
            });
        }
        (self.share() <= 0.0).then_some(Outcome::TimedOut)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Repair,
    Laser,
    Missiles,
    Side,
    Drone,
    SuperCharge,
    SlowChip,
}

const ITEMS: [Item; 7] = [Item::Repair, Item::Laser, Item::Missiles, Item::Side, Item::Drone, Item::SuperCharge, Item::SlowChip];

/// Nach so vielen besiegten Bossen führt Quibbo Seitenkanonen und Begleitdrohnen. Bis dahin
/// stehen sie mit einem Hinweis im Laden, damit man weiß, worauf man hinarbeitet.
pub const SIDE_AFTER: usize = 1;
pub const DRONES_AFTER: usize = 2;
const SIDE_PRICE: u32 = 500;
const SIDE_PRICE_PER_LEVEL: u32 = 450;
const DRONE_PRICE: u32 = 900;

/// Was nach dem soundsovielten besiegten Boss neu in den Laden kommt.
pub fn new_goods(beaten: usize) -> Option<&'static str> {
    match beaten {
        SIDE_AFTER => Some("Seitenkanonen"),
        DRONES_AFTER => Some("Begleitdrohnen"),
        _ => None,
    }
}

/// Hinweis für Ware, die erst nach einem bestimmten Boss zu haben ist.
fn locked(after: usize) -> String {
    // „Der Kubus“ mitten im Satz klein: „sobald der Kubus besiegt ist“.
    let boss = LEVELS[after - 1].boss.name.replacen("Der ", "der ", 1);
    format!("kommt, sobald {boss} besiegt ist")
}

/// Was der Shop über das Schiff wissen muss, um Preise und Angebote zu bestimmen.
struct ShipState {
    health: f32,
    laser: u8,
    missiles: u8,
    side: u8,
    drones: u8,
    bought_charges: u8,
    chips: u32,
    beaten: usize,
}

impl ShipState {
    fn new(player: &Player, loadout: &Loadout, weapon: &SuperWeapon, chips: &SlowChips, campaign: &Campaign) -> Self {
        Self {
            health: player.health,
            laser: loadout.laser,
            missiles: loadout.missiles,
            side: loadout.side,
            drones: loadout.drones,
            bought_charges: weapon.bought,
            chips: chips.0,
            beaten: campaign.beaten,
        }
    }
}

impl Item {
    /// Name, Beschreibung und Listenpreis; `None`, wenn es nichts mehr zu kaufen gibt.
    fn offer(self, ship: &ShipState) -> (String, String, Option<u32>) {
        match self {
            Item::Repair => {
                let missing = (MAX_HEALTH - ship.health).max(0.0).ceil();
                let points = missing.min(REPAIR_STEP);
                let price = (points > 0.0).then_some(points as u32 * REPAIR_PER_POINT);
                let name = if points > 0.0 { format!("Reparatur +{points:.0}") } else { "Reparatur".to_string() };
                (name, format!("Schild jetzt {:.0}/{MAX_HEALTH:.0}", ship.health), price)
            }
            Item::Laser => {
                let next = ship.laser + 1;
                let price = (ship.laser < MAX_LEVEL).then_some(LASER_PRICE_PER_LEVEL * ship.laser as u32);
                (format!("Laser Stufe {next}"), "ein Strahl mehr".into(), price)
            }
            Item::Missiles => {
                let price = match ship.missiles {
                    0 => Some(MISSILE_LAUNCHER_PRICE),
                    m if m < MAX_LEVEL => Some(LASER_PRICE_PER_LEVEL * (m as u32 + 1)),
                    _ => None,
                };
                let name = if ship.missiles == 0 { "Raketenwerfer".to_string() } else { format!("Raketen Stufe {}", ship.missiles + 1) };
                (name, "zielsuchend, feuern mit dem Laser".into(), price)
            }
            Item::Side if ship.beaten < SIDE_AFTER => ("Seitenkanonen".into(), locked(SIDE_AFTER), None),
            Item::Side => {
                let price = match ship.side {
                    0 => Some(SIDE_PRICE),
                    s if s < MAX_SIDE => Some(SIDE_PRICE_PER_LEVEL * (s as u32 + 1)),
                    _ => None,
                };
                let name = if ship.side == 0 { "Seitenkanonen".to_string() } else { format!("Seitenkanonen Stufe {}", ship.side + 1) };
                (name, "feuern schräg nach oben und unten, gut gegen Geschütze".into(), price)
            }
            Item::Drone if ship.beaten < DRONES_AFTER => ("Begleitdrohne".into(), locked(DRONES_AFTER), None),
            Item::Drone => {
                let price = (ship.drones < MAX_DRONES).then_some(DRONE_PRICE * (ship.drones as u32 + 1));
                (
                    format!("Begleitdrohne {}/{MAX_DRONES}", ship.drones + 1),
                    "fliegt neben dem Schiff und feuert mit".into(),
                    price,
                )
            }
            Item::SuperCharge => {
                let price = (ship.bought_charges < MAX_BOUGHT_CHARGES).then_some(SUPER_PRICE);
                (
                    "Superwaffen-Ladung".into(),
                    format!("+1 für den nächsten Flug ({}/{MAX_BOUGHT_CHARGES} gekauft)", ship.bought_charges),
                    price,
                )
            }
            Item::SlowChip => (
                format!("Zeitlupen-Chips ×{CHIPS_PER_PACK}"),
                format!("+50 % Zeit für je eine Aufgabe (vorrätig: {})", ship.chips),
                Some(CHIP_PRICE),
            ),
        }
    }
}

enum View {
    Hub { selected: usize, level: usize },
    Training { run: Option<Run>, wait: f32, feedback: Option<(String, Color)>, earned: u32 },
    /// `intro`: Beim Betreten erklärt Quibbo zuerst, wie sein Laden funktioniert.
    Shop { selected: usize, haggle: Option<Haggle>, feedback: Option<(String, Color)>, intro: bool },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Neutral,
    Pleased,
    Offended,
}

/// Eine Verhandlung mit Quibbo: eine Aufgabe, danach steht seine Antwort kurz da.
struct Haggle {
    item: Item,
    run: Run,
    result: Option<(String, Color, f32)>,
}

#[derive(Resource)]
pub struct Shipyard {
    view: View,
    /// Rabatte in Prozent, die in diesem Besuch ausgehandelt wurden; gelten für einen Kauf.
    discounts: Vec<(Item, u32)>,
    /// Hier wurde schon verhandelt; ein zweites Mal lässt Quibbo nicht mit sich reden.
    haggled: Vec<Item>,
    /// Quibbos Laune und wie lange sie noch anhält.
    mood: (Mood, f32),
    /// Ein Mausklick auf eine Zeile, ausgewertet wie Enter.
    clicked: Option<usize>,
}

impl Shipyard {
    /// Die Maus zeigt auf eine Zeile der Übersicht oder des Ladens.
    fn selected(&self) -> Option<usize> {
        match &self.view {
            View::Hub { selected, .. } | View::Shop { selected, .. } => Some(*selected),
            View::Training { .. } => None,
        }
    }

    fn hover(&mut self, row: usize) {
        match &mut self.view {
            View::Hub { selected, .. } if *selected != row => *selected = row,
            View::Shop { selected, haggle: None, intro: false, feedback } if *selected != row => {
                *selected = row;
                *feedback = None;
            }
            _ => {}
        }
    }

    /// Lösung der gerade offenen Aufgabe, für automatische Testläufe.
    #[cfg(debug_assertions)]
    pub fn open_answer(&self) -> Option<u32> {
        current_run(self).map(|run| run.task.answer)
    }
}

fn discount(discounts: &[(Item, u32)], item: Item) -> u32 {
    discounts.iter().find(|(i, _)| *i == item).map(|(_, d)| *d).unwrap_or(0)
}

fn price(discounts: &[(Item, u32)], item: Item, list: u32) -> u32 {
    list * (100 - discount(discounts, item)) / 100
}

#[derive(Component)]
struct ShipyardRoot;

#[derive(Component)]
struct Content;

#[derive(Component)]
struct RunBar;

#[derive(Component)]
struct QuibboPortrait;

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

fn enter(
    mut commands: Commands,
    assets: Res<AssetServer>,
    quibbo: Res<QuibboArt>,
    current: Res<CurrentLevel>,
    campaign: Res<Campaign>,
    mut time: ResMut<Time<Virtual>>,
) {
    time.pause();
    // Vorgeschlagen wird das nächste Level, sofern es schon freigeschaltet ist.
    let level = (current.0 + 1).min(campaign.unlocked - 1);
    commands.insert_resource(Shipyard {
        view: View::Hub { selected: 0, level },
        discounts: Vec::new(),
        haggled: Vec::new(),
        mood: (Mood::Neutral, 0.0),
        clicked: None,
    });
    let root = commands
        .spawn((
            ShipyardRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                column_gap: Val::Px(24.0),
                ..default()
            },
            ImageNode {
                image: assets.load(BACKGROUND_IMAGE),
                image_mode: NodeImageMode::Stretch,
                color: Color::srgb(0.3, 0.3, 0.35),
                ..default()
            },
            GlobalZIndex(30),
        ))
        .id();
    // Quibbo steht neben dem Ladentisch; sichtbar nur im Laden.
    commands.spawn((
        QuibboPortrait,
        Node { width: Val::Px(260.0), height: Val::Px(260.0), display: Display::None, ..default() },
        ImageNode { image: quibbo.open.clone(), ..default() },
        ChildOf(root),
    ));
    commands.spawn((
        Content,
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Stretch,
            row_gap: Val::Px(8.0),
            width: Val::Px(640.0),
            padding: UiRect::all(Val::Px(28.0)),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(12.0)),
            ..default()
        },
        BorderColor::from(FRAME),
        BackgroundColor(PANEL),
        ChildOf(root),
    ));
}

fn leave(mut commands: Commands, roots: Query<Entity, With<ShipyardRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    commands.remove_resource::<Shipyard>();
}

/// Nach jedem Flug beginnt die Trainingsstaffel neu, und die Credits des Flugs sind abgerechnet.
fn new_visit_after_flight(mut started: MessageReader<LevelStarted>, mut training: ResMut<Training>, mut credits: ResMut<Credits>) {
    if started.read().last().is_some() {
        training.solved = 0;
        credits.flight = 0;
    }
}

fn pressed(keys: &ButtonInput<KeyCode>, a: KeyCode, b: KeyCode) -> bool {
    keys.just_pressed(a) || keys.just_pressed(b)
}

fn confirm(keys: &ButtonInput<KeyCode>) -> bool {
    pressed(keys, KeyCode::Enter, KeyCode::NumpadEnter)
}

fn update(
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    mut shipyard: ResMut<Shipyard>,
    mut credits: ResMut<Credits>,
    mut training: ResMut<Training>,
    mut topic: ResMut<Topic>,
    mut rng: ResMut<Rng>,
    mut loadout: ResMut<Loadout>,
    mut weapon: ResMut<SuperWeapon>,
    mut chips: ResMut<SlowChips>,
    mut player: Single<&mut Player>,
    campaign: Res<Campaign>,
    mut started: MessageWriter<LevelStarted>,
) {
    let dt = time.delta_secs();
    // Zeitbalken laufen jedes Bild; neu gezeichnet wird nur, wenn sich etwas Sichtbares ändert.
    let mut changed = keys.get_just_pressed().next().is_some();
    let yard = shipyard.bypass_change_detection();
    let clicked = yard.clicked.take();
    let confirm = |keys: &ButtonInput<KeyCode>| confirm(keys) || clicked.is_some();
    let mut next_view = None;

    match &mut yard.view {
        View::Hub { selected, level } => {
            if pressed(&keys, KeyCode::KeyW, KeyCode::ArrowUp) {
                *selected = (*selected + 2) % 3;
            }
            if pressed(&keys, KeyCode::KeyS, KeyCode::ArrowDown) {
                *selected = (*selected + 1) % 3;
            }
            if *selected == 2 {
                if pressed(&keys, KeyCode::KeyA, KeyCode::ArrowLeft) {
                    *level = level.saturating_sub(1);
                }
                if pressed(&keys, KeyCode::KeyD, KeyCode::ArrowRight) {
                    *level = (*level + 1).min(campaign.unlocked - 1);
                }
            }
            if confirm(&keys) {
                changed = true;
                match *selected {
                    0 => next_view = Some(View::Training { run: None, wait: 0.0, feedback: None, earned: 0 }),
                    1 => next_view = Some(View::Shop { selected: 0, haggle: None, feedback: None, intro: true }),
                    _ => {
                        started.write(LevelStarted(*level));
                    }
                }
            }
        }
        View::Training { run, wait, feedback, earned } => {
            if keys.just_pressed(KeyCode::Escape) {
                next_view = Some(View::Hub { selected: 0, level: 0 });
            } else if let Some(current) = run {
                if let Some(outcome) = current.feed(&keys, dt) {
                    topic.0.record(&current.task, &outcome);
                    let solution = current.task.solution();
                    match outcome {
                        Outcome::Correct { remaining_share } => {
                            let amount = ((TRAINING_BASE + TRAINING_SPEED * remaining_share) * training.factor()).round().max(1.0) as u32;
                            credits.bank += amount;
                            *earned += amount;
                            training.solved += 1;
                            let praise = if remaining_share >= FAST_SHARE { "Blitzschnell!" } else { "Richtig!" };
                            *feedback = Some((format!("{praise} +{amount} Credits"), GOOD));
                            *wait = CORRECT_PAUSE;
                        }
                        Outcome::TimedOut => {
                            *feedback = Some((format!("Zeit um: {solution}"), OKAY));
                            *wait = TIMEOUT_PAUSE;
                        }
                        Outcome::Wrong => {
                            *feedback = Some((format!("Falsch: {solution} · kurz durchatmen …"), BAD));
                            *wait = WRONG_PAUSE;
                        }
                    }
                    *run = None;
                    changed = true;
                }
            } else {
                *wait -= dt;
                if *wait <= 0.0 {
                    *run = Some(Run::new(topic.0.next(&mut rng)));
                    changed = true;
                }
            }
        }
        View::Shop { intro, .. } if *intro => {
            if keys.just_pressed(KeyCode::Escape) {
                next_view = Some(View::Hub { selected: 1, level: 0 });
            } else if confirm(&keys) || keys.just_pressed(KeyCode::Space) {
                *intro = false;
                changed = true;
            }
        }
        View::Shop { selected, haggle, feedback, .. } => {
            let ship = ShipState::new(&player, &loadout, &weapon, &chips, &campaign);
            if let Some(h) = haggle {
                if let Some((_, _, remaining)) = &mut h.result {
                    *remaining -= dt;
                    if *remaining <= 0.0 {
                        *haggle = None;
                        changed = true;
                    }
                } else if keys.just_pressed(KeyCode::Escape) {
                    *haggle = None;
                } else if let Some(outcome) = h.run.feed(&keys, dt) {
                    topic.0.record(&h.run.task, &outcome);
                    let (discount, text, color) = match outcome {
                        Outcome::Correct { remaining_share } => {
                            let discount = 3 + (7.0 * remaining_share).round() as u32;
                            let text = if remaining_share >= FAST_SHARE {
                                format!("„Beeindruckend schnell! {discount} % Rabatt.“")
                            } else {
                                format!("„Richtig gerechnet. {discount} % Rabatt.“")
                            };
                            (discount, text, GOOD)
                        }
                        Outcome::TimedOut => (1, format!("„Zu langsam … {} – na gut, 1 % Rabatt.“", h.run.task.solution()), OKAY),
                        Outcome::Wrong => {
                            (0, format!("„Falsch! {} – darüber rede ich nicht mehr.“", h.run.task.solution()), BAD)
                        }
                    };
                    yard.haggled.push(h.item);
                    let mood = match outcome {
                        Outcome::Correct { .. } => Mood::Pleased,
                        Outcome::TimedOut => Mood::Neutral,
                        Outcome::Wrong => Mood::Offended,
                    };
                    yard.mood = (mood, MOOD_TIME);
                    if discount > 0 {
                        yard.discounts.push((h.item, discount));
                    }
                    h.result = Some((text, color, HAGGLE_RESULT_TIME));
                    changed = true;
                }
            } else {
                if keys.just_pressed(KeyCode::Escape) {
                    next_view = Some(View::Hub { selected: 1, level: 0 });
                }
                if pressed(&keys, KeyCode::KeyW, KeyCode::ArrowUp) {
                    *selected = (*selected + ITEMS.len() - 1) % ITEMS.len();
                    *feedback = None;
                }
                if pressed(&keys, KeyCode::KeyS, KeyCode::ArrowDown) {
                    *selected = (*selected + 1) % ITEMS.len();
                    *feedback = None;
                }
                let item = ITEMS[*selected];
                let (name, _, list) = item.offer(&ship);
                if keys.just_pressed(KeyCode::KeyV) {
                    *feedback = if list.is_none() {
                        Some(("Das gibt es gerade nicht.".into(), DIM))
                    } else if yard.haggled.contains(&item) {
                        Some(("„Über diesen Artikel haben wir schon geredet.“".into(), OKAY))
                    } else {
                        *haggle = Some(Haggle { item, run: Run::new(topic.0.next(&mut rng)), result: None });
                        None
                    };
                }
                if confirm(&keys) {
                    changed = true;
                    *feedback = Some(match list {
                        None => ("Das gibt es gerade nicht.".into(), DIM),
                        Some(list) => {
                            let price = price(&yard.discounts, item, list);
                            if credits.bank < price {
                                (format!("Zu wenig Credits für {name}: {price} nötig."), BAD)
                            } else {
                                credits.bank -= price;
                                // Ein Rabatt gilt für einen Kauf.
                                yard.discounts.retain(|(i, _)| *i != item);
                                match item {
                                    Item::Repair => player.health = (player.health + REPAIR_STEP).min(MAX_HEALTH),
                                    Item::Laser => loadout.laser += 1,
                                    Item::Missiles => loadout.missiles += 1,
                                    Item::Side => loadout.side += 1,
                                    Item::Drone => loadout.drones += 1,
                                    Item::SuperCharge => weapon.bought += 1,
                                    Item::SlowChip => chips.0 += CHIPS_PER_PACK,
                                }
                                yard.mood = (Mood::Pleased, MOOD_TIME);
                                (format!("Gekauft: {name} für {price} Credits."), GOOD)
                            }
                        }
                    });
                }
            }
        }
    }

    if yard.mood.0 != Mood::Neutral {
        yard.mood.1 -= dt;
        if yard.mood.1 <= 0.0 {
            yard.mood = (Mood::Neutral, 0.0);
            changed = true;
        }
    }
    if let Some(view) = next_view {
        // Zurück in die Übersicht mit dem vorgeschlagenen Level.
        let view = match view {
            View::Hub { selected, .. } => {
                let level = match &yard.view {
                    View::Hub { level, .. } => *level,
                    _ => campaign.unlocked - 1,
                };
                View::Hub { selected, level }
            }
            other => other,
        };
        yard.view = view;
        changed = true;
    }
    if changed {
        shipyard.set_changed();
    }
}

fn current_run(shipyard: &Shipyard) -> Option<&Run> {
    match &shipyard.view {
        View::Training { run, .. } => run.as_ref(),
        View::Shop { haggle: Some(h), .. } if h.result.is_none() => Some(&h.run),
        _ => None,
    }
}

fn update_bar(shipyard: Res<Shipyard>, mut bars: Query<(&mut Node, &mut BackgroundColor), With<RunBar>>) {
    let Some(run) = current_run(&shipyard) else {
        return;
    };
    let share = run.share();
    for (mut node, mut color) in &mut bars {
        node.width = Val::Percent(share * 100.0);
        color.0 = if share > FAST_SHARE { GOOD } else if share > 0.2 { OKAY } else { BAD };
    }
}

fn text(commands: &mut Commands, parent: Entity, value: impl Into<String>, color: Color, size: f32) {
    commands.spawn((Text::new(value), TextColor(color), font(size), ChildOf(parent)));
}

/// Eine Zeile, die auf die Maus reagiert: Zeigen wählt sie aus, Drücken bestätigt wie Enter.
/// Schon das Drücken zählt, weil die Zeile bis zum Loslassen neu aufgebaut sein kann.
fn clickable_row(commands: &mut Commands, parent: Entity, index: usize, selected: bool) -> Entity {
    let row = row(commands, parent, selected);
    commands
        .entity(row)
        .observe(move |press: On<PointerPress>, mut yard: ResMut<Shipyard>| {
            if press.button == PointerButton::Primary {
                yard.hover(index);
                yard.clicked = Some(index);
            }
        })
        .observe(move |_: On<PointerOver>, mut yard: ResMut<Shipyard>| {
            // Nur bei echtem Wechsel anfassen, sonst würde jedes Neuzeichnen das nächste auslösen.
            if yard.bypass_change_detection().selected() != Some(index) {
                yard.hover(index);
            }
        });
    row
}

fn row(commands: &mut Commands, parent: Entity, selected: bool) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(16.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BorderColor::from(if selected { FRAME } else { Color::NONE }),
            BackgroundColor(if selected { ROW_SELECTED } else { ROW }),
            ChildOf(parent),
        ))
        .id()
}

/// Eine Aufgabe mit Eingabe und Zeitbalken.
fn task_block(commands: &mut Commands, parent: Entity, run: &Run) {
    let block = commands
        .spawn((
            Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: Val::Px(8.0), ..default() },
            ChildOf(parent),
        ))
        .id();
    text(commands, block, run.task.with(&format!("{}_", run.input)), TEXT, 48.0);
    let bar = commands
        .spawn((
            Node { width: Val::Px(380.0), height: Val::Px(10.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(BAR_BACK),
            ChildOf(block),
        ))
        .id();
    commands.spawn((
        RunBar,
        Node {
            width: Val::Percent(run.share() * 100.0),
            height: Val::Percent(100.0),
            border_radius: BorderRadius::all(Val::Px(5.0)),
            ..default()
        },
        BackgroundColor(GOOD),
        ChildOf(bar),
    ));
}

fn render(
    mut commands: Commands,
    shipyard: Res<Shipyard>,
    credits: Res<Credits>,
    training: Res<Training>,
    loadout: Res<Loadout>,
    weapon: Res<SuperWeapon>,
    chips: Res<SlowChips>,
    player: Single<&Player>,
    campaign: Res<Campaign>,
    content: Single<Entity, With<Content>>,
    mut portrait: Single<(&mut Node, &mut ImageNode), With<QuibboPortrait>>,
) {
    if !shipyard.is_changed() {
        return;
    }
    let in_shop = matches!(shipyard.view, View::Shop { .. });
    portrait.0.display = if in_shop { Display::Flex } else { Display::None };
    portrait.1.color = match shipyard.mood.0 {
        Mood::Neutral => Color::WHITE,
        Mood::Pleased => Color::srgb(1.0, 1.0, 0.8),
        Mood::Offended => Color::srgb(1.0, 0.35, 0.35),
    };
    let content = *content;
    commands.entity(content).despawn_related::<Children>();
    let ship = ShipState::new(&player, &loadout, &weapon, &chips, &campaign);
    let c = &mut commands;

    match &shipyard.view {
        View::Hub { selected, level } => {
            text(c, content, "Werft", FRAME, 36.0);
            text(c, content, format!("{} Credits", credits.bank), OKAY, 24.0);
            let weapons = loadout.describe();
            text(c, content, format!("Schild {:.0}/{MAX_HEALTH:.0} · {weapons}", ship.health), TEXT, 18.0);
            let left = FULL_TASKS.saturating_sub(training.solved);
            let options = [
                format!("Rechentraining: Aufgaben lösen, Credits verdienen (noch {left} voll)"),
                "Quibbos Laden: Waffen, Reparatur und mehr kaufen".to_string(),
                format!("Abflug  ◀  Level {}: {} · {}  ▶", level + 1, LEVELS[*level].world, LEVELS[*level].topic),
            ];
            for (i, label) in options.into_iter().enumerate() {
                let r = clickable_row(c, content, i, i == *selected);
                text(c, r, label, TEXT, 20.0);
            }
            text(c, content, "W/S oder Maus wählen · A/D Level wechseln · Enter oder Klick bestätigen", DIM, 14.0);
        }
        View::Training { run, feedback, earned, .. } => {
            text(c, content, "Rechentraining", FRAME, 32.0);
            let rate = (training.factor() * 100.0).round();
            let status = if training.solved < FULL_TASKS {
                format!("Noch {} Aufgaben mit voller Auszahlung", FULL_TASKS - training.solved)
            } else {
                format!("Auszahlung jetzt {rate} % · nach dem nächsten Flug wieder voll")
            };
            text(c, content, format!("{} Credits · in dieser Runde +{earned}", credits.bank), OKAY, 20.0);
            text(c, content, status, DIM, 16.0);
            match run {
                Some(run) => task_block(c, content, run),
                None => text(c, content, "…", DIM, 48.0),
            }
            if let Some((message, color)) = feedback {
                text(c, content, message.clone(), *color, 18.0);
            }
            text(c, content, "Nummernblock · Enter · Esc zurück zur Werft", DIM, 14.0);
        }
        View::Shop { intro: true, .. } => {
            text(c, content, "Quibbos Laden", FRAME, 32.0);
            text(c, content, "„Willkommen in meinem Laden, Pilot! So läuft das hier:“", OKAY, 18.0);
            let rules = [
                "Credits verdienst du im Flug und im Rechentraining der Werft. Zu wenig Credits? Erst trainieren!",
                "W/S: Ware wählen · Enter: kaufen.",
                "V: verhandeln. Ich stelle dir eine Aufgabe. Schnell und richtig gibt bis zu 10 % Rabatt, Zeit um 1 %.",
                "Rechnest du falsch, rede ich über diese Ware nicht mehr. Einmal verhandeln je Ware und Besuch.",
                "Ein Rabatt gilt für einen Kauf.",
            ];
            for rule in rules {
                text(c, content, format!("• {rule}"), TEXT, 16.0);
            }
            text(c, content, format!("Du hast {} Credits.", credits.bank), OKAY, 18.0);
            let enter = clickable_row(c, content, 0, true);
            text(c, enter, "Laden betreten", TEXT, 20.0);
            text(c, content, "Enter oder Klick: Laden betreten · Esc: zurück zur Werft", DIM, 14.0);
        }
        View::Shop { selected, haggle, feedback, .. } => {
            text(c, content, "Quibbos Laden", FRAME, 32.0);
            let greeting = match shipyard.mood.0 {
                Mood::Neutral => "„Wer gut rechnet, zahlt weniger.“",
                Mood::Pleased => "„Ein Vergnügen, mit dir Geschäfte zu machen!“",
                Mood::Offended => "„Hmpf.“",
            };
            text(c, content, greeting, DIM, 16.0);
            text(c, content, format!("{} Credits", credits.bank), OKAY, 22.0);
            for (i, item) in ITEMS.iter().enumerate() {
                let (name, description, list) = item.offer(&ship);
                let r = clickable_row(c, content, i, i == *selected);
                let labels = c
                    .spawn((Node { flex_direction: FlexDirection::Column, ..default() }, ChildOf(r)))
                    .id();
                text(c, labels, name, if list.is_some() { TEXT } else { DIM }, 19.0);
                text(c, labels, description, DIM, 13.0);
                let (price, color) = match list {
                    None => ("—".to_string(), DIM),
                    Some(list) => {
                        let discount = discount(&shipyard.discounts, *item);
                        let price = price(&shipyard.discounts, *item, list);
                        let color = if price > credits.bank { BAD } else { OKAY };
                        if discount > 0 { (format!("{price} (−{discount} %)"), GOOD) } else { (price.to_string(), color) }
                    }
                };
                text(c, r, price, color, 19.0);
            }
            if let Some(h) = haggle {
                let panel = c
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::all(Val::Px(12.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BorderColor::from(OKAY),
                        BackgroundColor(ROW),
                        ChildOf(content),
                    ))
                    .id();
                match &h.result {
                    Some((message, color, _)) => text(c, panel, format!("Quibbo: {message}"), *color, 18.0),
                    None => {
                        text(c, panel, "Quibbo: „Beeindrucke mich!“", OKAY, 18.0);
                        task_block(c, panel, &h.run);
                    }
                }
            }
            if let Some((message, color)) = feedback {
                text(c, content, message.clone(), *color, 17.0);
            }
            text(c, content, "W/S oder Maus wählen · Enter oder Klick kaufen · V verhandeln · Esc zurück", DIM, 14.0);
        }
    }
}

/// Quibbo blinzelt ab und zu.
fn animate_quibbo(
    time: Res<Time<Real>>,
    art: Res<QuibboArt>,
    mut portrait: Single<&mut ImageNode, With<QuibboPortrait>>,
    mut clock: Local<f32>,
) {
    *clock = (*clock + time.delta_secs()) % BLINK_EVERY;
    let image = if *clock > BLINK_EVERY - BLINK_TIME { &art.blink } else { &art.open };
    if portrait.image != *image {
        portrait.image = image.clone();
    }
}
