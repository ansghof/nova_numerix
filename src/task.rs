//! Rechenaufgaben im Flug: Eine abgeschossene Frachtdrohne öffnet ein Fenster mit einer
//! Aufgabe, die Welt läuft derweil in Zeitlupe weiter. Danach beschleunigt sie in der
//! Rückkehr-Phase wieder, damit die rechte Hand Zeit hat, zurück an die Steuerung zu finden.
//!
//! Falsch ist immer schlechter als langsam: Wer richtig rechnet, bekommt die Belohnung und
//! Punkte fürs Tempo; wem die Zeit ausgeht, bekommt die Belohnung trotzdem; wer falsch rechnet,
//! bekommt nichts und einen leichten, nie tödlichen Treffer.

use bevy::prelude::*;

use crate::{GamePhase, Gameplay};
use crate::enemy::{FreightDestroyed, WaveCleared};
use crate::hud::Score;
use crate::level::{LevelStarted, LevelStats, Rewind};
use crate::shipyard::Credits;
use crate::neon::{CYAN, GREEN, MAGENTA, Shape, VIOLET, WHITE, polygon};
use crate::player::{Loadout, MAX_DRONES, MAX_LEVEL, MAX_SIDE, Player};
use crate::ship::Ship;
use crate::arsenal::{LIGHTNING, MAX_LIGHTNING, MAX_PLASMA};
use crate::shipyard::{DRONES_AFTER, LIGHTNING_AFTER, PLASMA_AFTER, SIDE_AFTER};
use crate::shots::PLASMA;
use crate::rng::Rng;
use crate::superweapon::FireSuper;

/// Weltgeschwindigkeit während einer Aufgabe.
const SLOW_MOTION: f32 = 0.1;
/// So lange (in echten Sekunden) bremst die Welt beim Öffnen auf Zeitlupe ab.
const SLOW_DOWN_TIME: f32 = 0.2;
/// So lange beschleunigt sie danach wieder auf volles Tempo.
const RETURN_TIME: f32 = 0.7;
/// Anschließend blinkt das Schiff noch so lange unverwundbar.
const AFTER_RETURN_INVULNERABLE: f32 = 1.0;
const WRONG_DAMAGE: f32 = 10.0;
const REPAIR_AMOUNT: f32 = 35.0;
pub const BASE_POINTS: f32 = 100.0;
pub const SPEED_POINTS: f32 = 400.0;
/// Ab diesem Anteil an Restzeit gilt eine Antwort als blitzschnell.
pub const FAST_SHARE: f32 = 0.5;
/// Die Superwaffe hat etwas mehr Zeit, weil ein Fehler eine zweite Aufgabe nach sich zieht.
const SUPER_EXTRA_TIME: f32 = 3.0;
/// Credits für jede richtig gelöste Aufgabe im Flug.
pub const TASK_CREDITS: u32 = 10;
const CHIP_TIME_FACTOR: f32 = 1.5;

const TEXT: Color = Color::srgb(0.92, 0.96, 1.0);
const DIM: Color = Color::srgb(0.55, 0.62, 0.75);
const GOOD: Color = Color::srgb(0.45, 1.0, 0.6);
const OKAY: Color = Color::srgb(1.0, 0.85, 0.35);
const BAD: Color = Color::srgb(1.0, 0.4, 0.4);
const FRAME: Color = Color::srgb(0.3, 0.85, 1.0);

pub struct TaskPlugin;

impl Plugin for TaskPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Topic(Box::new(SmallTimesTable::new()), 1.0))
            .init_resource::<PendingTasks>()
            .init_resource::<SlowChips>()
            .add_systems(Update, reset_on_start)
            .add_systems(OnEnter(GamePhase::Task), spawn_popup)
            .add_systems(OnExit(GamePhase::Return), close_popup)
            .add_systems(
                Update,
                (
                    count_freight,
                    start_task.run_if(in_state(GamePhase::Flying)),
                    type_answer.run_if(in_state(GamePhase::Task)),
                    finish_return.run_if(in_state(GamePhase::Return)),
                    world_speed,
                )
                    .chain()
                    .in_set(Gameplay),
            );
    }
}

#[derive(Default)]
pub struct Task {
    /// Die Aufgabe mit `?` an der gesuchten Stelle, etwa „7 × 8 = ?“ oder „3 × ? = 15“.
    pub prompt: String,
    /// Die gesuchte Zahl; bei Brüchen kodiert, siehe [`fraction`].
    pub answer: i32,
    /// Kennung des Rechenfakts, damit schwache Fakten öfter wiederkommen.
    pub fact: usize,
    /// Zeitlimit in echten Sekunden.
    pub time_limit: f32,
    /// Wie die Antwort geschrieben und eingetippt wird.
    pub style: AnswerStyle,
    /// Eine Merkhilfe unter der Aufgabe, etwa die Stellenwerte der Binärzahlen.
    pub hint: Option<&'static str>,
}

impl Task {
    /// Die Aufgabe mit `fill` an der Stelle der Lücke.
    pub fn with(&self, fill: &str) -> String {
        self.prompt.replace('?', fill)
    }

    /// Die vollständige, richtige Gleichung.
    pub fn solution(&self) -> String {
        self.with(&self.show(self.answer))
    }

    /// Eine Zahl so, wie sie als Antwort aussieht: Binärzahlen vierstellig mit führenden Nullen,
    /// römische Zahlen in Buchstaben, Brüche mit Bruchstrich.
    pub fn show(&self, value: i32) -> String {
        match self.style {
            AnswerStyle::Number | AnswerStyle::Signed => value.to_string(),
            AnswerStyle::Binary => format!("{value:04}"),
            AnswerStyle::Roman => roman(value.max(0) as u32),
            AnswerStyle::Fraction => {
                let (numerator, denominator) = split_fraction(value);
                format!("{numerator}/{denominator}")
            }
        }
    }

    /// Ein Zeichen der Eingabe: Ziffern, bei römischen Antworten die Buchstaben I, V, X, L, C,
    /// bei negativen Zahlen das Minus, bei Brüchen der Bruchstrich.
    fn read_key(&self, keys: &ButtonInput<KeyCode>) -> Option<char> {
        let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
        match self.style {
            AnswerStyle::Roman => {
                const LETTERS: [(KeyCode, char); 5] =
                    [(KeyCode::KeyI, 'I'), (KeyCode::KeyV, 'V'), (KeyCode::KeyX, 'X'), (KeyCode::KeyL, 'L'), (KeyCode::KeyC, 'C')];
                LETTERS.iter().find(|(key, _)| keys.just_pressed(*key)).map(|(_, c)| *c)
            }
            // Das Minus liegt je nach Tastatur woanders: Nummernblock, „-“ auf der deutschen
            // Tastatur (dort, wo auf der englischen „/“ sitzt) oder „-“ auf der englischen.
            AnswerStyle::Signed
                if keys.any_just_pressed([KeyCode::NumpadSubtract, KeyCode::Slash, KeyCode::Minus]) =>
            {
                Some('-')
            }
            // Bruchstrich: „/“ auf dem Nummernblock, Umschalt+7 auf der deutschen Tastatur oder
            // einfach die Leertaste.
            AnswerStyle::Fraction
                if keys.any_just_pressed([KeyCode::NumpadDivide, KeyCode::Space])
                    || (shift && keys.just_pressed(KeyCode::Digit7)) =>
            {
                Some('/')
            }
            _ => read_digit(keys),
        }
    }

    /// Verarbeitet die Tasten dieses Frames: ein neues Zeichen, sofern es an dieser Stelle Sinn
    /// ergibt, und die Rücktaste.
    pub fn type_into(&self, input: &mut String, keys: &ButtonInput<KeyCode>) {
        if let Some(key) = self.read_key(keys) {
            let fits = match key {
                // Das Minus nur ganz vorn.
                '-' => input.is_empty(),
                // Ein Bruchstrich, und erst nach dem Zähler.
                '/' => !input.is_empty() && !input.contains('/'),
                _ => true,
            };
            if fits && input.len() < self.max_len() {
                input.push(key);
            }
        }
        if keys.just_pressed(KeyCode::Backspace) {
            input.pop();
        }
    }

    /// Womit man die Antwort eintippt.
    pub fn input_hint(&self) -> &'static str {
        match self.style {
            AnswerStyle::Roman => "Tasten I V X L C",
            AnswerStyle::Signed => "Ziffern · Minus-Taste",
            AnswerStyle::Fraction => "Ziffern · Bruchstrich: / oder Leertaste",
            _ => "Nummernblock",
        }
    }

    /// Längste sinnvolle Eingabe; LXXXVIII (88) hat acht Zeichen, „-20“ drei, „12/25“ fünf.
    fn max_len(&self) -> usize {
        match self.style {
            AnswerStyle::Roman => 9,
            AnswerStyle::Fraction => 5,
            _ => 4,
        }
    }

    /// Ob die Eingabe stimmt. Römisch zählt nur die übliche Schreibweise: XL, nicht XXXX. Brüche
    /// müssen ganz gekürzt sein: 4/6 hat zwar den Wert von 2/3, ist aber nicht fertig.
    pub fn check(&self, input: &str) -> bool {
        match self.style {
            AnswerStyle::Roman => input == self.show(self.answer),
            AnswerStyle::Fraction => input == self.show(self.answer),
            _ => input.parse::<i32>() == Ok(self.answer),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum AnswerStyle {
    #[default]
    Number,
    /// Eine Binärzahl aus Nullen und Einsen, etwa 101 für „0101₂“; vierstellig angezeigt,
    /// führende Nullen darf man weglassen.
    Binary,
    /// Eine römische Zahl, getippt mit den Buchstabentasten.
    Roman,
    /// Eine ganze Zahl, die auch negativ sein kann; das Minus kommt zuerst.
    Signed,
    /// Ein gekürzter Bruch wie „2/3“; die Antwort ist mit [`fraction`] kodiert.
    Fraction,
}

/// Ein Bruch als eine Zahl, damit er wie jede andere Antwort durchs Spiel wandern kann:
/// 2/3 → 203. Nenner bleiben unter 100.
pub fn fraction(numerator: i32, denominator: i32) -> i32 {
    numerator * 100 + denominator
}

/// Umkehrung von [`fraction`]: 203 → (2, 3).
pub fn split_fraction(value: i32) -> (i32, i32) {
    (value / 100, value % 100)
}

/// Größter gemeinsamer Teiler.
pub fn gcd(a: i32, b: i32) -> i32 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// Eine Zahl von 1 bis 3999 in römischer Schreibweise: 47 → XLVII.
pub fn roman(mut n: u32) -> String {
    const PARTS: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut out = String::new();
    for (value, letters) in PARTS {
        while n >= value {
            out += letters;
            n -= value;
        }
    }
    out
}

/// Eine Zahl von 0 bis 15 als Ziffernfolge ihrer Binärdarstellung: 5 → 101.
pub fn binary_digits(n: u32) -> u32 {
    format!("{n:b}").parse().unwrap_or(0)
}

/// Umkehrung von [`binary_digits`]: 101 → 5; `None`, wenn andere Ziffern als 0 und 1 vorkommen.
pub fn from_binary_digits(digits: u32) -> Option<u32> {
    u32::from_str_radix(&digits.to_string(), 2).ok()
}

pub enum Outcome {
    Correct { remaining_share: f32 },
    TimedOut,
    Wrong,
}

/// Ein Thema, aus dem Aufgaben gezogen werden. Ein neues Level-Thema ist eine neue
/// Implementierung.
pub trait TaskGenerator: Send + Sync + 'static {
    fn next(&mut self, rng: &mut Rng) -> Task;
    /// Wie die Aufgabe ausging, damit das Thema schwache Fakten öfter stellt.
    fn record(&mut self, task: &Task, outcome: &Outcome);
    /// Der Lernstand zum Speichern: ein Gewicht je Fakt.
    fn weights(&self) -> Vec<f32>;
    /// Einen gespeicherten Lernstand übernehmen; passt die Länge nicht, bleibt alles beim Alten.
    fn set_weights(&mut self, weights: &[f32]);
}

fn restore(target: &mut [f32], weights: &[f32]) {
    if target.len() == weights.len() {
        target.copy_from_slice(weights);
    }
}

/// Kleines Einmaleins von 2 × 2 bis 10 × 10. Jedes Fakt hat ein Gewicht: Fehler erhöhen es,
/// schnelle richtige Antworten senken es. Ein Teil der Aufgaben fragt nach einer Lücke im
/// Faktor („3 × ? = 15“). Aufgaben mit 10 sind zu leicht und kommen anfangs selten.
pub struct SmallTimesTable {
    weights: Vec<f32>,
    last: Option<usize>,
}

impl SmallTimesTable {
    const LOW: u32 = 2;
    const SPAN: usize = 9;

    /// Anteil der Lückenaufgaben.
    const GAP_SHARE: f32 = 0.35;
    /// Startgewicht der Aufgaben mit 10 im Vergleich zu den anderen.
    const TEN_WEIGHT: f32 = 0.25;

    pub fn new() -> Self {
        let weights = (0..Self::SPAN * Self::SPAN)
            .map(|fact| {
                let (a, b) = Self::factors(fact);
                if a == 10 || b == 10 { Self::TEN_WEIGHT } else { 1.0 }
            })
            .collect();
        Self { weights, last: None }
    }

    fn factors(fact: usize) -> (u32, u32) {
        (Self::LOW + (fact / Self::SPAN) as u32, Self::LOW + (fact % Self::SPAN) as u32)
    }

    /// Dieselbe Aufgabe wie zuletzt, auch vertauscht: Nach 10 × 3 soll nicht 3 × 10 kommen.
    fn repeats_last(&self, fact: usize) -> bool {
        self.last.is_some_and(|last| {
            let swapped = (last % Self::SPAN) * Self::SPAN + last / Self::SPAN;
            fact == last || fact == swapped
        })
    }
}

impl TaskGenerator for SmallTimesTable {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let fact = pick_weighted(&self.weights, |i| self.repeats_last(i), rng);
        self.last = Some(fact);
        let (a, b) = Self::factors(fact);
        let product = a * b;
        if rng.chance(Self::GAP_SHARE) {
            // Lücken brauchen einen Gedanken mehr, also eine Sekunde mehr Zeit.
            let (prompt, answer) =
                if rng.chance(0.5) { (format!("{a} × ? = {product}"), b) } else { (format!("? × {b} = {product}"), a) };
            Task { prompt, answer: answer as i32, fact, time_limit: 7.0, ..Default::default() }
        } else {
            Task { prompt: format!("{a} × {b} = ?"), answer: product as i32, fact, time_limit: 6.0, ..Default::default() }
        }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        reweigh(&mut self.weights[task.fact], outcome);
    }

    fn weights(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn set_weights(&mut self, weights: &[f32]) {
        restore(&mut self.weights, weights);
    }
}

/// Quadratzahlen von 2² bis 15². Lückenaufgaben fragen nach der Zahl, die quadriert wurde
/// („?² = 144“). 10² ist zu leicht und kommt anfangs selten.
pub struct Squares {
    weights: Vec<f32>,
    last: Option<usize>,
}

impl Squares {
    const LOW: u32 = 2;
    const HIGH: u32 = 15;
    const GAP_SHARE: f32 = 0.35;

    pub fn new() -> Self {
        let weights = (Self::LOW..=Self::HIGH).map(|n| if n == 10 { 0.25 } else { 1.0 }).collect();
        Self { weights, last: None }
    }
}

impl TaskGenerator for Squares {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let fact = pick_weighted(&self.weights, |i| Some(i) == self.last, rng);
        self.last = Some(fact);
        let n = Self::LOW + fact as u32;
        let square = n * n;
        if rng.chance(Self::GAP_SHARE) {
            Task { prompt: format!("?² = {square}"), answer: n as i32, fact, time_limit: 8.0, ..Default::default() }
        } else {
            Task { prompt: format!("{n}² = ?"), answer: square as i32, fact, time_limit: 7.0, ..Default::default() }
        }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        reweigh(&mut self.weights[task.fact], outcome);
    }

    fn weights(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn set_weights(&mut self, weights: &[f32]) {
        restore(&mut self.weights, weights);
    }
}

/// Großes Einmaleins: 11 bis 20 mal 2 bis 9. Leichte Fakten (mit 11, 20 oder 2) kommen
/// anfangs seltener. Lückenaufgaben fragen nach einem Faktor („13 × ? = 78“).
pub struct BigTimesTable {
    weights: Vec<f32>,
    last: Option<usize>,
}

impl BigTimesTable {
    const BIG: std::ops::RangeInclusive<u32> = 11..=20;
    const SMALL_LOW: u32 = 2;
    const SMALL_SPAN: usize = 8;
    const GAP_SHARE: f32 = 0.3;

    pub fn new() -> Self {
        let weights = (0..10 * Self::SMALL_SPAN)
            .map(|fact| {
                let (a, b) = Self::factors(fact);
                if a == 11 || a == 20 || b == 2 { 0.4 } else { 1.0 }
            })
            .collect();
        Self { weights, last: None }
    }

    fn factors(fact: usize) -> (u32, u32) {
        (*Self::BIG.start() + (fact / Self::SMALL_SPAN) as u32, Self::SMALL_LOW + (fact % Self::SMALL_SPAN) as u32)
    }
}

impl TaskGenerator for BigTimesTable {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let fact = pick_weighted(&self.weights, |i| Some(i) == self.last, rng);
        self.last = Some(fact);
        let (a, b) = Self::factors(fact);
        let product = a * b;
        if rng.chance(Self::GAP_SHARE) {
            let (prompt, answer) =
                if rng.chance(0.5) { (format!("{a} × ? = {product}"), b) } else { (format!("? × {b} = {product}"), a) };
            Task { prompt, answer: answer as i32, fact, time_limit: 12.0, ..Default::default() }
        } else {
            // Mal steht die große Zahl vorn, mal hinten.
            let prompt = if rng.chance(0.5) { format!("{a} × {b} = ?") } else { format!("{b} × {a} = ?") };
            Task { prompt, answer: product as i32, fact, time_limit: 11.0, ..Default::default() }
        }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        reweigh(&mut self.weights[task.fact], outcome);
    }

    fn weights(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn set_weights(&mut self, weights: &[f32]) {
        restore(&mut self.weights, weights);
    }
}

/// Binärzahlen mit vier Bit (1 bis 15) in beide Richtungen: „1011₂ = ?“ wird als Dezimalzahl
/// beantwortet, „11 = ?₂“ mit Nullen und Einsen auf dem Nummernblock. 1, 2, 4 und 8 sind leicht
/// und kommen anfangs seltener.
pub struct BinaryNumbers {
    weights: Vec<f32>,
    last: Option<usize>,
}

impl BinaryNumbers {
    const HINT: &'static str = "Stellenwerte: 8 · 4 · 2 · 1";

    pub fn new() -> Self {
        let weights = (0..30)
            .map(|fact| {
                let n = fact as u32 / 2 + 1;
                if n.is_power_of_two() { 0.4 } else { 1.0 }
            })
            .collect();
        Self { weights, last: None }
    }
}

impl TaskGenerator for BinaryNumbers {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let fact = pick_weighted(&self.weights, |i| Some(i) == self.last, rng);
        self.last = Some(fact);
        let n = fact as u32 / 2 + 1;
        let hint = Some(Self::HINT);
        if fact % 2 == 0 {
            Task { prompt: format!("{n:04b}₂ = ?"), answer: n as i32, fact, time_limit: 10.0, hint, ..Default::default() }
        } else {
            let answer = binary_digits(n) as i32;
            Task { prompt: format!("{n} = ?₂"), answer, fact, time_limit: 12.0, style: AnswerStyle::Binary, hint }
        }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        reweigh(&mut self.weights[task.fact], outcome);
    }

    fn weights(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn set_weights(&mut self, weights: &[f32]) {
        restore(&mut self.weights, weights);
    }
}

/// Römische Zahlen von 1 bis 100 in beide Richtungen: „XLVII = ?“ wird mit Ziffern beantwortet,
/// „47 = ?“ mit den Buchstabentasten I, V, X, L und C. Leichte Zahlen (die Grundzeichen und ihre
/// einfachen Wiederholungen wie III oder XX) kommen anfangs seltener; Zahlen mit IV, IX, XL und XC
/// sind die eigentliche Übung.
pub struct RomanNumerals {
    weights: Vec<f32>,
    last: Option<usize>,
}

impl RomanNumerals {
    const HINT: &'static str = "I = 1 · V = 5 · X = 10 · L = 50 · C = 100";

    pub fn new() -> Self {
        let weights = (0..200)
            .map(|fact| {
                let n = fact as u32 / 2 + 1;
                let letters = roman(n);
                let subtractive = ["IV", "IX", "XL", "XC"].iter().any(|s| letters.contains(s));
                if subtractive {
                    1.4
                } else if letters.len() <= 2 {
                    0.4
                } else {
                    1.0
                }
            })
            .collect();
        Self { weights, last: None }
    }
}

impl TaskGenerator for RomanNumerals {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let fact = pick_weighted(&self.weights, |i| Some(i) == self.last, rng);
        self.last = Some(fact);
        let n = fact as u32 / 2 + 1;
        if fact % 2 == 0 {
            Task { prompt: format!("{} = ?", roman(n)), answer: n as i32, fact, time_limit: 10.0, hint: Some(Self::HINT), ..Default::default() }
        } else {
            Task {
                prompt: format!("{n} = ?"),
                answer: n as i32,
                fact,
                time_limit: 14.0,
                style: AnswerStyle::Roman,
                hint: Some(Self::HINT),
            }
        }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        reweigh(&mut self.weights[task.fact], outcome);
    }

    fn weights(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn set_weights(&mut self, weights: &[f32]) {
        restore(&mut self.weights, weights);
    }
}

/// Addition mit negativen Zahlen: zwei Summanden von −10 bis 10, mindestens einer negativ, etwa
/// „−7 + 12 = ?“ oder „5 + (−9) = ?“. Ergebnisse liegen zwischen −20 und 20 und werden mit der
/// Minus-Taste eingetippt. Aufgaben, bei denen das Vorzeichen wechselt, sind die eigentliche Übung
/// und kommen etwas öfter.
pub struct NegativeNumbers {
    pairs: Vec<(i32, i32)>,
    weights: Vec<f32>,
    last: Option<usize>,
}

impl NegativeNumbers {
    const HINT: &'static str = "+ positive Zahl → rechts · + negative Zahl → links";

    pub fn new() -> Self {
        let range = (-10..=10).filter(|n: &i32| *n != 0);
        let pairs: Vec<(i32, i32)> =
            range.clone().flat_map(|a| range.clone().map(move |b| (a, b))).filter(|(a, b)| *a < 0 || *b < 0).collect();
        let weights = pairs
            .iter()
            .map(|(a, b)| {
                let sum = a + b;
                if sum.signum() != a.signum() { 1.3 } else if *a < 0 && *b < 0 { 0.8 } else { 1.0 }
            })
            .collect();
        Self { pairs, weights, last: None }
    }
}

impl TaskGenerator for NegativeNumbers {
    fn next(&mut self, rng: &mut Rng) -> Task {
        let fact = pick_weighted(&self.weights, |i| Some(i) == self.last, rng);
        self.last = Some(fact);
        let (a, b) = self.pairs[fact];
        // Ein negativer zweiter Summand steht in Klammern: „5 + (−9)“.
        let second = if b < 0 { format!("({b})") } else { b.to_string() };
        Task {
            prompt: format!("{a} + {second} = ?"),
            answer: a + b,
            fact,
            time_limit: 9.0,
            style: AnswerStyle::Signed,
            hint: Some(Self::HINT),
        }
    }

    fn record(&mut self, task: &Task, outcome: &Outcome) {
        reweigh(&mut self.weights[task.fact], outcome);
    }

    fn weights(&self) -> Vec<f32> {
        self.weights.clone()
    }

    fn set_weights(&mut self, weights: &[f32]) {
        restore(&mut self.weights, weights);
    }
}

/// Zieht ein Fakt nach Gewicht; `skip` schließt etwa die gerade gestellte Aufgabe aus.
fn pick_weighted(weights: &[f32], skip: impl Fn(usize) -> bool, rng: &mut Rng) -> usize {
    let total: f32 = weights.iter().enumerate().filter(|(i, _)| !skip(*i)).map(|(_, w)| w).sum();
    let mut pick = rng.f32() * total;
    let mut fact = 0;
    for (i, w) in weights.iter().enumerate() {
        if skip(i) {
            continue;
        }
        fact = i;
        if pick < *w {
            break;
        }
        pick -= w;
    }
    fact
}

/// Fehler machen ein Fakt häufiger, schnelle richtige Antworten seltener.
fn reweigh(weight: &mut f32, outcome: &Outcome) {
    *weight = match outcome {
        Outcome::Correct { remaining_share } if *remaining_share >= FAST_SHARE => *weight * 0.6,
        Outcome::Correct { .. } => *weight * 0.85,
        Outcome::TimedOut => *weight * 1.6,
        Outcome::Wrong => *weight * 2.5,
    }
    .clamp(0.2, 8.0);
}

/// Zeitlupen-Chips aus dem Shop: Jeder gibt einer Aufgabe im Flug 50 % mehr Zeit.
#[derive(Resource, Default)]
pub struct SlowChips(pub u32);

#[derive(Resource)]
/// Das laufende Rechenthema und der Zeitfaktor des Schwierigkeitsgrads.
pub struct Topic(pub Box<dyn TaskGenerator>, pub f32);

impl Topic {
    /// Die nächste Aufgabe, mit dem Zeitlimit des Schwierigkeitsgrads.
    pub fn next(&mut self, rng: &mut Rng) -> Task {
        let mut task = self.0.next(rng);
        task.time_limit *= self.1;
        task
    }
}

/// Aufgaben, die starten, sobald gerade keine läuft.
#[derive(Resource, Default)]
pub struct PendingTasks {
    /// Ladung abgeschossener Frachtdrohnen.
    freight: Vec<Reward>,
    /// Der Spieler will die Superwaffe zünden.
    pub super_weapon: bool,
    /// Größen ausgelöschter Wellen, deren Bonusaufgabe noch aussteht.
    waves: Vec<u32>,
}

impl PendingTasks {
    /// Am Levelende verfallen offene Aufgaben.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Wofür gerechnet wird.
enum Purpose {
    Freight(Reward),
    /// Bei einem Fehler gibt es eine zweite Aufgabe; die Zeit läuft dabei weiter.
    Super { second_chance_used: bool },
    /// Eine Welle wurde ganz abgeschossen; richtig gerechnet gibt es Sonderpunkte.
    WaveBonus { size: u32 },
}

/// Sonderpunkte je Gegner der Welle; Tempo verdreifacht sie höchstens.
const WAVE_POINTS_PER_ENEMY: f32 = 60.0;
/// Mehr Bonusaufgaben werden nicht aufgehoben, wenn gerade viel los ist.
const MAX_PENDING_WAVES: usize = 2;

fn wave_points(size: u32, share: f32) -> u32 {
    (size as f32 * WAVE_POINTS_PER_ENEMY * (1.0 + 2.0 * share)).round() as u32
}

/// Was eine Frachtdrohne geladen hat. Steht schon beim Anflug fest und ist auf ihr zu sehen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reward {
    Laser,
    Missiles,
    Repair,
    Side,
    Drone,
    Plasma,
    Lightning,
}

impl Reward {
    pub fn name(self) -> &'static str {
        match self {
            Self::Laser => "Laser +1",
            Self::Missiles => "Raketen +1",
            Self::Repair => "Reparatur",
            Self::Side => "Seitenkanone +1",
            Self::Drone => "Begleitdrohne",
            Self::Plasma => "Plasma +1",
            Self::Lightning => "Blitzbogen +1",
        }
    }

    /// Kurzer Name für das Schild unter der Frachtdrohne.
    pub fn label(self) -> &'static str {
        match self {
            Self::Laser => "Laser",
            Self::Missiles => "Raketen",
            Self::Repair => "Reparatur",
            Self::Side => "Seitenkanone",
            Self::Drone => "Drohne",
            Self::Plasma => "Plasma",
            Self::Lightning => "Blitz",
        }
    }

    pub fn color(self) -> LinearRgba {
        match self {
            Self::Laser => WHITE,
            Self::Missiles => CYAN,
            Self::Repair => GREEN,
            Self::Side => VIOLET,
            Self::Drone => MAGENTA,
            Self::Plasma => PLASMA,
            Self::Lightning => LIGHTNING,
        }
    }

    /// Das Zeichen im Inneren der Frachtdrohne.
    pub fn icon(self) -> Shape {
        let shape = Shape::new(self.color());
        match self {
            // Drei Strahlen
            Self::Laser => shape
                .open(&[Vec2::new(-8.0, 5.0), Vec2::new(8.0, 5.0)])
                .open(&[Vec2::new(-8.0, 0.0), Vec2::new(8.0, 0.0)])
                .open(&[Vec2::new(-8.0, -5.0), Vec2::new(8.0, -5.0)]),
            // Eine Rakete
            Self::Missiles => shape
                .closed(&[Vec2::new(9.0, 0.0), Vec2::new(-5.0, 5.0), Vec2::new(-5.0, -5.0)])
                .open(&[Vec2::new(-5.0, 0.0), Vec2::new(-10.0, 0.0)]),
            // Ein Pluszeichen
            Self::Repair => shape
                .closed(&[
                    Vec2::new(-3.0, 9.0),
                    Vec2::new(3.0, 9.0),
                    Vec2::new(3.0, 3.0),
                    Vec2::new(9.0, 3.0),
                    Vec2::new(9.0, -3.0),
                    Vec2::new(3.0, -3.0),
                    Vec2::new(3.0, -9.0),
                    Vec2::new(-3.0, -9.0),
                    Vec2::new(-3.0, -3.0),
                    Vec2::new(-9.0, -3.0),
                    Vec2::new(-9.0, 3.0),
                    Vec2::new(-3.0, 3.0),
                ]),
            // Zwei schräge Rohre
            Self::Side => shape
                .open(&[Vec2::new(-7.0, 0.0), Vec2::new(7.0, 8.0)])
                .open(&[Vec2::new(-7.0, 0.0), Vec2::new(7.0, -8.0)])
                .closed(&polygon(4, 3.0, 0.0)),
            // Eine kleine Drohne
            Self::Drone => shape.closed(&polygon(4, 9.0, 0.0)).closed(&polygon(4, 3.5, std::f32::consts::FRAC_PI_4)),
            // Zwei Plasmabögen
            Self::Plasma => shape
                .open(&[Vec2::new(-4.0, -8.0), Vec2::new(2.0, -4.0), Vec2::new(4.0, 0.0), Vec2::new(2.0, 4.0), Vec2::new(-4.0, 8.0)])
                .open(&[Vec2::new(2.0, -8.0), Vec2::new(8.0, -4.0), Vec2::new(10.0, 0.0), Vec2::new(8.0, 4.0), Vec2::new(2.0, 8.0)]),
            // Ein Blitz
            Self::Lightning => shape.open(&[
                Vec2::new(3.0, 10.0),
                Vec2::new(-4.0, 1.0),
                Vec2::new(2.0, 1.0),
                Vec2::new(-3.0, -10.0),
            ]),
        }
    }

    /// Eine Belohnung, die gerade etwas bringt: keine Waffe über der Höchststufe, Reparatur
    /// bevorzugt bei angeschlagenem Schiff. Neue Waffen gibt es, sobald Quibbo sie auch führt
    /// und am Rumpf ein Waffenplatz frei ist.
    pub fn choose(rng: &mut Rng, loadout: &Loadout, player: &Player, platforms: usize, beaten: usize) -> Self {
        if player.health < player.max_health * 0.5 && rng.chance(0.6) {
            return Self::Repair;
        }
        let mut options = Vec::new();
        if loadout.laser < MAX_LEVEL {
            options.push(Self::Laser);
        }
        if loadout.missiles < MAX_LEVEL && loadout.fits(loadout.missiles, platforms) {
            options.push(Self::Missiles);
        }
        if beaten >= SIDE_AFTER && loadout.side < MAX_SIDE && loadout.fits(loadout.side, platforms) {
            options.push(Self::Side);
        }
        // Drohnen sind selten: nur jede dritte Wahl darf eine sein.
        if beaten >= DRONES_AFTER
            && loadout.drones < MAX_DRONES
            && loadout.fits(loadout.drones, platforms)
            && rng.chance(0.33)
        {
            options.push(Self::Drone);
        }
        if beaten >= PLASMA_AFTER && loadout.plasma < MAX_PLASMA && loadout.fits(loadout.plasma, platforms) {
            options.push(Self::Plasma);
        }
        if beaten >= LIGHTNING_AFTER && loadout.lightning < MAX_LIGHTNING && loadout.fits(loadout.lightning, platforms) && rng.chance(0.5) {
            options.push(Self::Lightning);
        }
        if options.is_empty() { Self::Repair } else { options[rng.below(options.len())] }
    }

    /// Gibt zurück, was tatsächlich an Bord kam: Ist für eine neue Waffe inzwischen kein Platz
    /// mehr frei, wird daraus eine Reparatur.
    fn apply(self, loadout: &mut Loadout, player: &mut Player, platforms: usize) -> Self {
        let current = match self {
            Self::Missiles => Some(loadout.missiles),
            Self::Side => Some(loadout.side),
            Self::Drone => Some(loadout.drones),
            Self::Plasma => Some(loadout.plasma),
            Self::Lightning => Some(loadout.lightning),
            _ => None,
        };
        if current.is_some_and(|level| !loadout.fits(level, platforms)) {
            return Self::Repair.apply(loadout, player, platforms);
        }
        match self {
            Self::Laser => loadout.laser = (loadout.laser + 1).min(MAX_LEVEL),
            Self::Missiles => loadout.missiles = (loadout.missiles + 1).min(MAX_LEVEL),
            Self::Repair => player.health = (player.health + REPAIR_AMOUNT).min(player.max_health),
            Self::Side => loadout.side = (loadout.side + 1).min(MAX_SIDE),
            Self::Drone => loadout.drones = (loadout.drones + 1).min(MAX_DRONES),
            Self::Plasma => loadout.plasma = (loadout.plasma + 1).min(MAX_PLASMA),
            Self::Lightning => loadout.lightning = (loadout.lightning + 1).min(MAX_LIGHTNING),
        }
        self
    }
}

#[derive(Resource)]
pub struct ActiveTask {
    pub task: Task,
    purpose: Purpose,
    input: String,
    /// Verstrichene echte Zeit seit dem Öffnen.
    elapsed: f32,
    time_limit: f32,
    /// Verstrichene echte Zeit in der Rückkehr-Phase.
    returning: f32,
    /// Ein Zeitlupen-Chip verlängert diese Aufgabe.
    chip: bool,
}

#[derive(Component)]
struct Popup;

#[derive(Component)]
struct PromptText;

#[derive(Component)]
struct TimeBar;

#[derive(Component)]
struct HintText;

fn font(size: f32) -> TextFont {
    TextFont {
        font: crate::fonts::family(),
        ..TextFont::from_font_size(size)
    }
}

/// Neues Level oder Neustart am Checkpoint: offene Aufgaben verfallen, ein noch offenes Fenster
/// verschwindet.
fn reset_on_start(
    mut commands: Commands,
    mut started: MessageReader<LevelStarted>,
    mut rewind: MessageReader<Rewind>,
    mut pending: ResMut<PendingTasks>,
    popups: Query<Entity, With<Popup>>,
) {
    if started.read().count() + rewind.read().count() == 0 {
        return;
    }
    pending.clear();
    for popup in &popups {
        commands.entity(popup).despawn();
    }
    commands.remove_resource::<ActiveTask>();
}

fn count_freight(
    mut freight: MessageReader<FreightDestroyed>,
    mut cleared: MessageReader<WaveCleared>,
    mut pending: ResMut<PendingTasks>,
) {
    pending.freight.extend(freight.read().map(|f| f.0));
    for wave in cleared.read() {
        if pending.waves.len() < MAX_PENDING_WAVES {
            pending.waves.push(wave.size);
        }
    }
}

fn start_task(
    mut commands: Commands,
    mut pending: ResMut<PendingTasks>,
    mut topic: ResMut<Topic>,
    mut rng: ResMut<Rng>,
    mut chips: ResMut<SlowChips>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    let (purpose, extra_time) = if pending.super_weapon {
        pending.super_weapon = false;
        (Purpose::Super { second_chance_used: false }, SUPER_EXTRA_TIME)
    } else if !pending.freight.is_empty() {
        (Purpose::Freight(pending.freight.remove(0)), 0.0)
    } else if !pending.waves.is_empty() {
        (Purpose::WaveBonus { size: pending.waves.remove(0) }, 0.0)
    } else {
        return;
    };
    let task = topic.next(&mut rng);
    let mut time_limit = task.time_limit + extra_time;
    let chip = chips.0 > 0;
    if chip {
        chips.0 -= 1;
        time_limit *= CHIP_TIME_FACTOR;
    }
    commands.insert_resource(ActiveTask { task, purpose, input: String::new(), elapsed: 0.0, time_limit, returning: 0.0, chip });
    next.set(GamePhase::Task);
}

fn spawn_popup(mut commands: Commands, active: Res<ActiveTask>) {
    let root = commands
        .spawn((
            Popup,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                // Unter dem Boss-Banner, damit beide lesbar bleiben.
                top: Val::Px(150.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                min_width: Val::Px(440.0),
                padding: UiRect::axes(Val::Px(32.0), Val::Px(20.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BorderColor::from(FRAME),
            BackgroundColor(Color::srgba(0.02, 0.04, 0.12, 0.88)),
            ChildOf(root),
        ))
        .id();
    let title = match &active.purpose {
        Purpose::Freight(reward) => format!("Fracht gesichert · Belohnung: {}", reward.name()),
        Purpose::Super { .. } => "Superwaffe laden · je schneller, desto stärker".to_string(),
        Purpose::WaveBonus { size } => format!("Welle ausgelöscht! Bonus bis +{}", wave_points(*size, 1.0)),
    };
    commands.spawn((
        Text::new(title),
        TextColor(OKAY),
        font(18.0),
        ChildOf(panel),
    ));
    commands.spawn((
        PromptText,
        Text::new(active.task.with("_")),
        TextColor(TEXT),
        font(56.0),
        ChildOf(panel),
    ));
    if let Some(topic_hint) = active.task.hint {
        commands.spawn((Text::new(topic_hint), TextColor(OKAY), font(18.0), ChildOf(panel)));
    }
    let bar = commands
        .spawn((
            Node { width: Val::Px(380.0), height: Val::Px(12.0), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
            BackgroundColor(Color::srgb(0.08, 0.1, 0.2)),
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        TimeBar,
        Node { width: Val::Percent(100.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
        BackgroundColor(GOOD),
        ChildOf(bar),
    ));
    let keys = active.task.input_hint();
    let hint = if active.chip { format!("{keys} · Enter · Zeitlupen-Chip: +50 % Zeit") } else { format!("{keys} · Enter") };
    commands.spawn((HintText, Text::new(hint), TextColor(DIM), font(16.0), ChildOf(panel)));
}

impl ActiveTask {
    fn remaining_share(&self) -> f32 {
        (1.0 - self.elapsed / self.time_limit).clamp(0.0, 1.0)
    }
}

/// Ziffern über Nummernblock oder Ziffernreihe, Rücktaste löscht, Enter gibt ab.
pub fn read_digit(keys: &ButtonInput<KeyCode>) -> Option<char> {
    const KEYS: [(KeyCode, KeyCode); 10] = [
        (KeyCode::Digit0, KeyCode::Numpad0),
        (KeyCode::Digit1, KeyCode::Numpad1),
        (KeyCode::Digit2, KeyCode::Numpad2),
        (KeyCode::Digit3, KeyCode::Numpad3),
        (KeyCode::Digit4, KeyCode::Numpad4),
        (KeyCode::Digit5, KeyCode::Numpad5),
        (KeyCode::Digit6, KeyCode::Numpad6),
        (KeyCode::Digit7, KeyCode::Numpad7),
        (KeyCode::Digit8, KeyCode::Numpad8),
        (KeyCode::Digit9, KeyCode::Numpad9),
    ];
    KEYS.iter()
        .position(|(a, b)| keys.just_pressed(*a) || keys.just_pressed(*b))
        .and_then(|d| char::from_digit(d as u32, 10))
}

fn type_answer(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut active: ResMut<ActiveTask>,
    mut topic: ResMut<Topic>,
    mut score: ResMut<Score>,
    mut loadout: ResMut<Loadout>,
    ship: Res<Ship>,
    mut rng: ResMut<Rng>,
    mut stats: ResMut<LevelStats>,
    mut credits: ResMut<Credits>,
    mut fire: MessageWriter<FireSuper>,
    mut player: Single<&mut Player>,
    mut prompt: Single<(&mut Text, &mut TextColor), (With<PromptText>, Without<HintText>)>,
    mut hint: Single<(&mut Text, &mut TextColor), (With<HintText>, Without<PromptText>)>,
    mut bar: Single<(&mut Node, &mut BackgroundColor), With<TimeBar>>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    active.elapsed += time.delta_secs();
    let active = &mut *active;
    active.task.type_into(&mut active.input, &keys);

    let share = active.remaining_share();
    bar.0.width = Val::Percent(share * 100.0);
    bar.1.0 = if share > FAST_SHARE { GOOD } else if share > 0.2 { OKAY } else { BAD };
    prompt.0.0 = active.task.with(&format!("{}_", active.input));

    let submitted = (keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)) && !active.input.is_empty();
    let outcome = if submitted {
        if active.task.check(&active.input) {
            Outcome::Correct { remaining_share: share }
        } else {
            Outcome::Wrong
        }
    } else if share <= 0.0 {
        Outcome::TimedOut
    } else {
        return;
    };

    let solution = active.task.solution();
    topic.0.record(&active.task, &outcome);
    stats.count(&outcome);
    if matches!(outcome, Outcome::Correct { .. }) {
        credits.flight += TASK_CREDITS;
    }
    let (color, feedback) = match (&mut active.purpose, outcome) {
        (Purpose::Freight(reward), Outcome::Correct { remaining_share }) => {
            let points = (BASE_POINTS + SPEED_POINTS * remaining_share).round() as u32;
            score.0 += points;
            let got = reward.apply(&mut loadout, &mut player, ship.hull.platforms());
            let praise = if remaining_share >= FAST_SHARE { "Blitzschnell!" } else { "Sauber gerechnet!" };
            (GOOD, format!("{praise} +{points} Punkte · {}", got.name()))
        }
        (Purpose::Freight(reward), Outcome::TimedOut) => {
            let got = reward.apply(&mut loadout, &mut player, ship.hull.platforms());
            (OKAY, format!("Zeit um – {} gibt's trotzdem. Nächstes Mal schneller!", got.name()))
        }
        (Purpose::Freight(_), Outcome::Wrong) => {
            // Ein Fehler kostet etwas Schild, aber nie das Schiff.
            player.health = (player.health - WRONG_DAMAGE).max(1.0);
            (BAD, format!("Knapp daneben – richtig ist {}", active.task.show(active.task.answer)))
        }
        (Purpose::WaveBonus { size }, Outcome::Correct { remaining_share }) => {
            let points = wave_points(*size, remaining_share);
            score.0 += points;
            let praise = if remaining_share >= FAST_SHARE { "Blitzschnell!" } else { "Sauber gerechnet!" };
            (GOOD, format!("{praise} Sonderpunkte +{points}"))
        }
        (Purpose::WaveBonus { .. }, Outcome::TimedOut) => {
            (OKAY, "Zeit um – diesmal kein Bonus. Nächstes Mal schneller!".to_string())
        }
        (Purpose::WaveBonus { .. }, Outcome::Wrong) => {
            player.health = (player.health - WRONG_DAMAGE).max(1.0);
            (BAD, format!("Knapp daneben – richtig ist {}", active.task.show(active.task.answer)))
        }
        (Purpose::Super { .. }, Outcome::Correct { remaining_share }) => {
            let points = (BASE_POINTS + SPEED_POINTS * remaining_share).round() as u32;
            score.0 += points;
            let fast = remaining_share >= FAST_SHARE;
            fire.write(FireSuper { tier: if fast { 3 } else { 2 }, power: remaining_share });
            let praise = if fast { "Volle Ladung!" } else { "Superwaffe gezündet!" };
            (GOOD, format!("{praise} +{points} Punkte"))
        }
        (Purpose::Super { second_chance_used }, Outcome::Wrong) if !*second_chance_used => {
            // Zweite Chance: neue Aufgabe, der Balken läuft einfach weiter.
            *second_chance_used = true;
            let message = format!("Falsch: {solution}. Neue Aufgabe, die Zeit läuft weiter!");
            active.task = topic.next(&mut rng);
            active.input.clear();
            hint.0.0 = message;
            hint.1.0 = BAD;
            return;
        }
        (Purpose::Super { .. }, Outcome::Wrong) => {
            player.health = (player.health - WRONG_DAMAGE).max(1.0);
            fire.write(FireSuper { tier: 1, power: 0.0 });
            (BAD, format!("Fehlzündung – richtig ist {}. Nur ein schwacher Puls.", active.task.show(active.task.answer)))
        }
        (Purpose::Super { .. }, Outcome::TimedOut) => {
            fire.write(FireSuper { tier: 1, power: 0.0 });
            (OKAY, "Zeit um – ein schwacher Puls. Nächstes Mal schneller!".to_string())
        }
    };
    prompt.0.0 = solution;
    prompt.1.0 = color;
    hint.0.0 = feedback;
    hint.1.0 = color;
    next.set(GamePhase::Return);
}

fn finish_return(
    time: Res<Time<Real>>,
    mut active: ResMut<ActiveTask>,
    mut player: Single<&mut Player>,
    mut next: ResMut<NextState<GamePhase>>,
) {
    active.returning += time.delta_secs();
    if active.returning >= RETURN_TIME {
        player.invulnerable = AFTER_RETURN_INVULNERABLE;
        next.set(GamePhase::Flying);
    }
}

fn close_popup(mut commands: Commands, popups: Query<Entity, With<Popup>>) {
    for popup in &popups {
        commands.entity(popup).despawn();
    }
    commands.remove_resource::<ActiveTask>();
}

/// Bremst die Welt beim Öffnen einer Aufgabe ab und beschleunigt sie danach sanft wieder.
fn world_speed(
    real: Res<Time<Real>>,
    phase: Res<State<GamePhase>>,
    active: Option<Res<ActiveTask>>,
    mut time: ResMut<Time<Virtual>>,
) {
    let speed = match (phase.get(), active) {
        (GamePhase::Task, _) => {
            let step = (1.0 - SLOW_MOTION) / SLOW_DOWN_TIME * real.delta_secs();
            (time.relative_speed() - step).max(SLOW_MOTION)
        }
        (GamePhase::Return, Some(active)) => {
            let t = (active.returning / RETURN_TIME).clamp(0.0, 1.0);
            let eased = t * t * (3.0 - 2.0 * t);
            SLOW_MOTION + (1.0 - SLOW_MOTION) * eased
        }
        _ => 1.0,
    };
    if speed != time.relative_speed() {
        time.set_relative_speed(speed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Setzt die Lösung ein und prüft, ob die Gleichung stimmt.
    fn holds(task: &Task) -> bool {
        let solution = task.solution();
        let (left, right) = solution.split_once(" = ").unwrap();
        let (a, b) = left.split_once(" × ").unwrap();
        a.parse::<u32>().unwrap() * b.parse::<u32>().unwrap() == right.parse::<u32>().unwrap()
    }

    #[test]
    fn einmaleins_mit_luecken_und_seltener_zehn() {
        let mut rng = Rng::default();
        let mut table = SmallTimesTable::new();
        let (mut gaps, mut tens) = (0, 0);
        let n = 4000;
        for _ in 0..n {
            let task = table.next(&mut rng);
            assert!(holds(&task), "{} mit {}", task.prompt, task.answer);
            if !task.prompt.ends_with('?') {
                gaps += 1;
            }
            if task.solution().split([' ', '×', '=']).any(|part| part == "10") {
                tens += 1;
            }
        }
        let gap_share = gaps as f32 / n as f32;
        assert!((0.3..0.4).contains(&gap_share), "Lückenanteil {gap_share}");
        // Ohne Bremse wären 17 von 81 Fakten mit 10 dabei, also etwa 21 %.
        let ten_share = tens as f32 / n as f32;
        assert!(ten_share < 0.1, "Anteil mit 10: {ten_share}");
    }
}

#[cfg(test)]
mod square_tests {
    use super::*;

    #[test]
    fn quadratzahlen_stimmen() {
        let mut rng = Rng::default();
        let mut squares = Squares::new();
        let mut gaps = 0;
        for _ in 0..2000 {
            let task = squares.next(&mut rng);
            let solution = task.solution();
            let (left, right) = solution.split_once("² = ").unwrap();
            let n: u32 = left.parse().unwrap();
            assert_eq!(n * n, right.parse::<u32>().unwrap(), "{}", task.prompt);
            assert!((2..=15).contains(&n));
            if task.prompt.starts_with('?') {
                gaps += 1;
            }
        }
        assert!((500..900).contains(&gaps), "Lücken: {gaps}");
    }
}

#[cfg(test)]
mod new_topic_tests {
    use super::*;

    #[test]
    fn grosses_einmaleins_stimmt() {
        let mut rng = Rng::default();
        let mut table = BigTimesTable::new();
        for _ in 0..2000 {
            let task = table.next(&mut rng);
            let solution = task.solution();
            let (left, right) = solution.split_once(" = ").unwrap();
            let (a, b) = left.split_once(" × ").unwrap();
            let (a, b): (u32, u32) = (a.parse().unwrap(), b.parse().unwrap());
            assert_eq!(a * b, right.parse::<u32>().unwrap(), "{}", task.prompt);
            assert!((11..=20).contains(&a.max(b)) && (2..=9).contains(&a.min(b)), "{}", task.prompt);
        }
    }

    #[test]
    fn binaerzahlen_stimmen() {
        let mut rng = Rng::default();
        let mut topic = BinaryNumbers::new();
        let mut binary_answers = 0;
        for _ in 0..2000 {
            let task = topic.next(&mut rng);
            let solution = task.solution();
            let (left, right) = solution.split_once(" = ").unwrap();
            let binary = task.style == AnswerStyle::Binary;
            let (bits, decimal) = if binary { (right, left) } else { (left, right) };
            let bits = bits.trim_end_matches('₂');
            assert_eq!(bits.len(), 4, "{solution}");
            assert_eq!(u32::from_str_radix(bits, 2).unwrap(), decimal.parse::<u32>().unwrap(), "{solution}");
            if binary {
                binary_answers += 1;
                // Führende Nullen darf man weglassen: „101“ ist dieselbe Eingabe wie „0101“.
                assert_eq!(bits.parse::<i32>().unwrap(), task.answer);
            }
        }
        assert!((700..1300).contains(&binary_answers), "{binary_answers}");
        assert_eq!(from_binary_digits(binary_digits(13)), Some(13));
    }
}

#[cfg(test)]
mod roman_tests {
    use super::*;

    #[test]
    fn roemische_zahlen() {
        assert_eq!(roman(4), "IV");
        assert_eq!(roman(9), "IX");
        assert_eq!(roman(14), "XIV");
        assert_eq!(roman(40), "XL");
        assert_eq!(roman(49), "XLIX");
        assert_eq!(roman(88), "LXXXVIII");
        assert_eq!(roman(90), "XC");
        assert_eq!(roman(99), "XCIX");
        assert_eq!(roman(100), "C");
        let longest = (1..=100).map(|n| roman(n).len()).max().unwrap();
        assert!(longest <= 9);
    }

    #[test]
    fn roemische_aufgaben() {
        let mut rng = Rng::default();
        let mut topic = RomanNumerals::new();
        let mut typed = 0;
        for _ in 0..2000 {
            let task = topic.next(&mut rng);
            assert!((1..=100).contains(&task.answer));
            assert!(task.check(&task.show(task.answer)), "{}", task.prompt);
            if task.style == AnswerStyle::Roman {
                typed += 1;
                // Nur die übliche Schreibweise zählt.
                assert!(!task.check(&"I".repeat(task.answer as usize)) || task.answer <= 3);
            } else {
                assert!(task.prompt.starts_with(&roman(task.answer as u32)));
            }
        }
        assert!((700..1300).contains(&typed), "{typed}");
    }
}

#[cfg(test)]
mod signed_tests {
    use super::*;

    #[test]
    fn negative_zahlen() {
        let mut rng = Rng::default();
        let mut topic = NegativeNumbers::new();
        let mut negative = 0;
        for _ in 0..2000 {
            let task = topic.next(&mut rng);
            let (left, right) = task.prompt.split_once(" = ").unwrap();
            assert_eq!(right, "?");
            let (a, b) = left.split_once(" + ").unwrap();
            let a: i32 = a.parse().unwrap();
            let b: i32 = b.trim_matches(['(', ')']).parse().unwrap();
            assert!(a < 0 || b < 0, "{}", task.prompt);
            assert_eq!(a + b, task.answer);
            assert!(task.check(&task.answer.to_string()));
            if task.answer < 0 {
                negative += 1;
            }
        }
        assert!((600..1400).contains(&negative), "{negative}");
    }

    #[test]
    fn minus_nur_vorn() {
        let task = NegativeNumbers::new().next(&mut Rng::default());
        let mut keys = ButtonInput::<KeyCode>::default();
        let mut input = String::new();
        for key in [KeyCode::Numpad3, KeyCode::NumpadSubtract, KeyCode::Backspace, KeyCode::NumpadSubtract] {
            keys.press(key);
            task.type_into(&mut input, &keys);
            keys.release(key);
            keys.clear();
        }
        assert_eq!(input, "-");
    }
}
