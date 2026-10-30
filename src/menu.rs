//! Splash beim Start, danach das Hauptmenü mit dem Titelbild: weiterfliegen, neues Spiel in
//! einem der fünf Slots (mit Schiffsnamen), Spielstand laden oder beenden. Esc im Flug führt
//! zurück ins Menü; die Welt steht still, solange es offen ist.
//!
//! Bedienung mit Tastatur (W/S oder Pfeiltasten, Enter, Esc) oder Maus. Die Knöpfe werden nur
//! neu aufgebaut, wenn sich der Inhalt der Seite ändert; die Auswahl färbt sie bloß um. Sonst
//! verschwände der Knopf unter dem Mauszeiger, bevor der Klick ihn erreicht.

use bevy::app::AppExit;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::AppState;
use crate::save::{DEFAULT_SHIP_NAME, MAX_NAME_LENGTH, SLOTS, Saves, load_game, new_game};

/// Bilder relativ zum Ordner `assets`. Fehlt das Splash-Bild, zeigt der Splash das Menübild.
const SPLASH_IMAGE: &str = "splash.png";
const MENU_IMAGE: &str = "menu.png";
/// So lange steht der Splash, bevor das Menü kommt; eine Taste überspringt ihn.
const SPLASH_SECONDS: f32 = 3.0;
const FADE_SECONDS: f32 = 0.6;

const TEXT: Color = Color::srgb(0.92, 0.96, 1.0);
const DIM: Color = Color::srgb(0.55, 0.62, 0.75);
const WARN: Color = Color::srgb(1.0, 0.85, 0.35);
const BUTTON: Color = Color::srgba(0.03, 0.05, 0.15, 0.85);
const SELECTED: Color = Color::srgba(0.1, 0.25, 0.45, 0.92);
const FRAME: Color = Color::srgb(0.3, 0.85, 1.0);
const FRAME_DIM: Color = Color::srgba(0.3, 0.85, 1.0, 0.25);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlightStarted>()
            .add_systems(OnEnter(AppState::Splash), spawn_splash)
            .add_systems(OnExit(AppState::Splash), despawn::<SplashScreen>)
            .add_systems(OnEnter(AppState::Menu), (spawn_menu, pause_world))
            .add_systems(OnExit(AppState::Menu), (despawn::<MenuScreen>, resume_world))
            .add_systems(OnEnter(AppState::Playing), |mut started: ResMut<FlightStarted>| started.0 = true)
            .add_systems(
                Update,
                (
                    (fallback_splash, run_splash).run_if(in_state(AppState::Splash)),
                    (navigate, type_name, render, highlight).chain().run_if(in_state(AppState::Menu)),
                    open_menu.run_if(in_state(AppState::Playing)),
                ),
            );
    }
}

#[derive(Component)]
struct SplashScreen;

#[derive(Component)]
struct SplashImage;

#[derive(Component)]
struct MenuScreen;

/// Die Spalte mit den Knöpfen; sie wird bei jeder Änderung neu gefüllt.
#[derive(Component)]
struct MenuColumn;

#[derive(Component)]
struct MenuRow(usize);

/// Ob schon geflogen wurde; dann gibt es „Weiterfliegen“.
#[derive(Resource, Default)]
struct FlightStarted(bool);

#[derive(Clone, Copy, PartialEq, Eq)]
enum MainAction {
    Continue,
    NewGame,
    Load,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotMode {
    NewGame,
    Load,
}

enum Page {
    Main { selected: usize },
    Slots { mode: SlotMode, selected: usize, confirm_overwrite: bool, note: Option<String> },
    Name { slot: usize, name: String },
}

#[derive(Resource)]
struct Menu {
    page: Page,
    /// Ein Mausklick auf eine Zeile, ausgewertet wie Enter.
    clicked: Option<usize>,
}

impl Menu {
    /// Die hervorgehobene Zeile; das Namensfeld ist immer hervorgehoben.
    fn selected(&self) -> usize {
        match &self.page {
            Page::Main { selected } | Page::Slots { selected, .. } => *selected,
            Page::Name { .. } => 0,
        }
    }

    /// Die Maus zeigt auf eine Zeile.
    fn hover(&mut self, row: usize) {
        match &mut self.page {
            Page::Main { selected } if *selected != row => *selected = row,
            Page::Slots { selected, confirm_overwrite, note, .. } if *selected != row => {
                *selected = row;
                *confirm_overwrite = false;
                *note = None;
            }
            _ => {}
        }
    }

    /// Alles, was die Seite ausmacht, außer der Auswahl. Ändert es sich, wird neu aufgebaut.
    fn layout(&self) -> String {
        match &self.page {
            Page::Main { .. } => "main".into(),
            Page::Slots { mode, confirm_overwrite, note, .. } => {
                format!("slots {} {confirm_overwrite} {note:?}", *mode == SlotMode::NewGame)
            }
            Page::Name { slot, name } => format!("name {slot} {name}"),
        }
    }
}

fn font(size: f32) -> TextFont {
    TextFont {
        font: FontSource::families("'Segoe UI', 'Noto Sans', sans-serif"),
        ..TextFont::from_font_size(size)
    }
}

fn fullscreen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        ..default()
    }
}

fn despawn<T: Component>(mut commands: Commands, screens: Query<Entity, With<T>>) {
    for screen in &screens {
        commands.entity(screen).despawn();
    }
}

fn spawn_splash(mut commands: Commands, assets: Res<AssetServer>) {
    let screen = commands
        .spawn((SplashScreen, fullscreen(), BackgroundColor(Color::BLACK), GlobalZIndex(50)))
        .id();
    commands.spawn((
        SplashImage,
        fullscreen(),
        ImageNode {
            image: assets.load(SPLASH_IMAGE),
            image_mode: NodeImageMode::Stretch,
            color: Color::WHITE.with_alpha(0.0),
            ..default()
        },
        ChildOf(screen),
    ));
}

/// Gibt es noch kein eigenes Splash-Bild, springt das Menübild ein.
fn fallback_splash(assets: Res<AssetServer>, mut image: Single<&mut ImageNode, With<SplashImage>>, mut checked: Local<bool>) {
    if *checked || !assets.load_state(&image.image).is_failed() {
        return;
    }
    *checked = true;
    image.image = assets.load(MENU_IMAGE);
}

fn run_splash(
    time: Res<Time<Real>>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut image: Single<&mut ImageNode, With<SplashImage>>,
    mut next: ResMut<NextState<AppState>>,
    mut shown: Local<f32>,
) {
    *shown += time.delta_secs();
    let fade_in = (*shown / FADE_SECONDS).min(1.0);
    let fade_out = ((SPLASH_SECONDS - *shown) / FADE_SECONDS).clamp(0.0, 1.0);
    image.color = Color::WHITE.with_alpha(fade_in.min(fade_out));
    let skipped = keys.get_just_pressed().next().is_some() || mouse.get_just_pressed().next().is_some();
    if *shown >= SPLASH_SECONDS || skipped {
        next.set(AppState::Menu);
    }
}

fn main_actions(started: bool) -> Vec<MainAction> {
    let mut actions = Vec::new();
    if started {
        actions.push(MainAction::Continue);
    }
    actions.extend([MainAction::NewGame, MainAction::Load, MainAction::Quit]);
    actions
}

fn spawn_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(Menu { page: Page::Main { selected: 0 }, clicked: None });
    let screen = commands
        .spawn((
            MenuScreen,
            fullscreen(),
            ImageNode { image: assets.load(MENU_IMAGE), image_mode: NodeImageMode::Stretch, ..default() },
            GlobalZIndex(40),
        ))
        .id();
    // Die Knöpfe sitzen in der freien Mitte des Bildes, unter dem Schriftzug.
    commands.spawn((
        MenuColumn,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            top: Val::Percent(48.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(10.0),
            ..default()
        },
        ChildOf(screen),
    ));
}

fn row_count(page: &Page, started: bool) -> usize {
    match page {
        Page::Main { .. } => main_actions(started).len(),
        Page::Slots { .. } => SLOTS,
        Page::Name { .. } => 0,
    }
}

fn navigate(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<Menu>,
    started: Res<FlightStarted>,
    saves: Res<Saves>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let pressed = |a: KeyCode, b: KeyCode| keys.just_pressed(a) || keys.just_pressed(b);
    let clicked = menu.clicked.take();
    let count = row_count(&menu.page, started.0);
    let up = pressed(KeyCode::KeyW, KeyCode::ArrowUp);
    let down = pressed(KeyCode::KeyS, KeyCode::ArrowDown);
    let confirm = pressed(KeyCode::Enter, KeyCode::NumpadEnter) || clicked.is_some();
    let back = keys.just_pressed(KeyCode::Escape);

    let mut new_page = None;
    match &mut menu.page {
        Page::Main { selected } => {
            if up {
                *selected = (*selected + count - 1) % count;
            }
            if down {
                *selected = (*selected + 1) % count;
            }
            if let Some(row) = clicked {
                *selected = row;
            }
            if confirm {
                match main_actions(started.0)[*selected] {
                    MainAction::Continue => next.set(AppState::Playing),
                    MainAction::NewGame => {
                        new_page = Some(Page::Slots { mode: SlotMode::NewGame, selected: 0, confirm_overwrite: false, note: None })
                    }
                    MainAction::Load => {
                        let first = saves.overview().iter().position(Option::is_some).unwrap_or(0);
                        new_page = Some(Page::Slots { mode: SlotMode::Load, selected: first, confirm_overwrite: false, note: None });
                    }
                    MainAction::Quit => {
                        exit.write(AppExit::Success);
                    }
                }
            }
        }
        Page::Slots { mode, selected, confirm_overwrite, note } => {
            if up || down {
                *selected = if up { (*selected + SLOTS - 1) % SLOTS } else { (*selected + 1) % SLOTS };
                *confirm_overwrite = false;
                *note = None;
            }
            if let Some(row) = clicked
                && row != *selected
            {
                *selected = row;
                *confirm_overwrite = false;
            }
            if back {
                new_page = Some(Page::Main { selected: 0 });
            } else if confirm {
                let occupied = saves.overview()[*selected].is_some();
                match mode {
                    SlotMode::Load if occupied => commands.run_system_cached_with(load_game, *selected),
                    SlotMode::Load => *note = Some("Dieser Slot ist leer.".into()),
                    SlotMode::NewGame if occupied && !*confirm_overwrite => *confirm_overwrite = true,
                    SlotMode::NewGame => new_page = Some(Page::Name { slot: *selected, name: DEFAULT_SHIP_NAME.into() }),
                }
            }
        }
        Page::Name { slot, name } => {
            if back {
                new_page = Some(Page::Slots { mode: SlotMode::NewGame, selected: *slot, confirm_overwrite: false, note: None });
            } else if (pressed(KeyCode::Enter, KeyCode::NumpadEnter) || clicked == Some(1)) && !name.trim().is_empty() {
                commands.run_system_cached_with(new_game, (*slot, name.trim().to_string()));
            }
        }
    }
    if let Some(page) = new_page {
        menu.page = page;
    }
}

/// Texteingabe für den Schiffsnamen, mit Umlauten und Leerzeichen.
fn type_name(mut input: MessageReader<KeyboardInput>, keys: Res<ButtonInput<KeyCode>>, mut menu: ResMut<Menu>) {
    let Page::Name { name, .. } = &mut menu.bypass_change_detection().page else {
        input.clear();
        return;
    };
    let mut changed = false;
    for event in input.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }
        if let Some(text) = &event.text {
            for c in text.chars().filter(|c| !c.is_control()) {
                if name.chars().count() < MAX_NAME_LENGTH {
                    name.push(c);
                    changed = true;
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::Backspace) {
        name.pop();
        changed = true;
    }
    if changed {
        menu.set_changed();
    }
}

fn text(commands: &mut Commands, parent: Entity, value: impl Into<String>, color: Color, size: f32) {
    commands.spawn((Text::new(value), TextColor(color), font(size), ChildOf(parent)));
}

fn row(commands: &mut Commands, parent: Entity, index: usize, selected: bool, width: f32) -> Entity {
    let row = commands
        .spawn((
            MenuRow(index),
            Node {
                width: Val::Px(width),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(22.0), Val::Px(8.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BorderColor::from(if selected { FRAME } else { FRAME_DIM }),
            BackgroundColor(if selected { SELECTED } else { BUTTON }),
            ChildOf(parent),
        ))
        .id();
    // Schon das Drücken zählt: Bis zum Loslassen kann die Seite gewechselt haben.
    commands
        .entity(row)
        .observe(move |press: On<PointerPress>, mut menu: ResMut<Menu>| {
            if press.button == PointerButton::Primary {
                menu.hover(index);
                menu.clicked = Some(index);
            }
        })
        .observe(move |_: On<PointerOver>, mut menu: ResMut<Menu>| menu.hover(index));
    row
}

/// Ein dunkles Schild hinter Hinweisen, damit sie auf dem bunten Bild lesbar bleiben.
fn hint(commands: &mut Commands, parent: Entity, value: &str) {
    let plate = commands
        .spawn((
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(BUTTON),
            ChildOf(parent),
        ))
        .id();
    text(commands, plate, value, DIM, 15.0);
}

fn render(
    mut commands: Commands,
    menu: Res<Menu>,
    started: Res<FlightStarted>,
    saves: Res<Saves>,
    column: Single<Entity, With<MenuColumn>>,
    mut built: Local<Option<(Entity, String)>>,
) {
    // Ein neu geöffnetes Menü hat eine neue Spalte und wird in jedem Fall aufgebaut.
    let layout = (*column, menu.layout());
    if built.as_ref() == Some(&layout) {
        return;
    }
    *built = Some(layout);
    let column = *column;
    commands.entity(column).despawn_related::<Children>();
    let c = &mut commands;

    match &menu.page {
        Page::Main { selected } => {
            for (i, action) in main_actions(started.0).into_iter().enumerate() {
                let label = match action {
                    MainAction::Continue => "Weiterfliegen",
                    MainAction::NewGame => "Neues Spiel",
                    MainAction::Load => "Spiel laden",
                    MainAction::Quit => "Beenden",
                };
                let r = row(c, column, i, i == *selected, 280.0);
                text(c, r, label, TEXT, 24.0);
            }
            hint(c, column, "W/S, Pfeiltasten oder Maus wählen · Enter oder Klick bestätigen · im Flug: Esc für dieses Menü");
        }
        Page::Slots { mode, selected, confirm_overwrite, note } => {
            let title = match mode {
                SlotMode::NewGame => "Neues Spiel – in welchen Slot?",
                SlotMode::Load => "Spiel laden",
            };
            text(c, column, title, TEXT, 24.0);
            for (i, slot) in saves.overview().into_iter().enumerate() {
                let r = row(c, column, i, i == *selected, 560.0);
                match slot {
                    Some((name, detail)) => {
                        text(c, r, format!("Slot {} · {name}", i + 1), TEXT, 19.0);
                        text(c, r, detail, DIM, 14.0);
                    }
                    None => text(c, r, format!("Slot {} · frei", i + 1), DIM, 19.0),
                }
            }
            if *confirm_overwrite {
                text(c, column, "Dieser Slot ist belegt: Enter überschreibt ihn, Esc bricht ab.", WARN, 17.0);
            } else if let Some(note) = note {
                hint(c, column, note);
            } else {
                hint(c, column, "W/S oder Maus wählen · Enter oder Klick bestätigen · Esc zurück");
            }
        }
        Page::Name { slot, name } => {
            text(c, column, format!("Slot {} · Wie heißt dein Schiff?", slot + 1), TEXT, 24.0);
            let field = row(c, column, 0, true, 420.0);
            text(c, field, format!("{name}_"), TEXT, 30.0);
            let go = row(c, column, 1, false, 280.0);
            text(c, go, "Losfliegen", TEXT, 22.0);
            hint(c, column, "Namen tippen · Rücktaste löscht · Enter: losfliegen · Esc zurück");
        }
    }
}

/// Die gewählte Zeile leuchtet, die anderen bleiben dunkel.
fn highlight(menu: Res<Menu>, mut rows: Query<(&MenuRow, &mut BorderColor, &mut BackgroundColor)>) {
    let selected = menu.selected();
    for (row, mut border, mut background) in &mut rows {
        let on = row.0 == selected;
        *border = BorderColor::from(if on { FRAME } else { FRAME_DIM });
        background.0 = if on { SELECTED } else { BUTTON };
    }
}

fn open_menu(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Menu);
    }
}

fn pause_world(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn resume_world(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}
