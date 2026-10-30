//! Splash beim Start, danach das Hauptmenü mit dem Titelbild. Esc im Flug führt zurück ins
//! Menü; die Welt steht still, solange es offen ist.
//!
//! Bedienung mit Tastatur (W/S oder Pfeiltasten, Enter) oder Maus.

use bevy::app::AppExit;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

use crate::AppState;

/// Bilder relativ zum Ordner `assets`. Fehlt das Splash-Bild, zeigt der Splash das Menübild.
const SPLASH_IMAGE: &str = "splash.png";
const MENU_IMAGE: &str = "menu.png";
/// So lange steht der Splash, bevor das Menü kommt; eine Taste überspringt ihn.
const SPLASH_SECONDS: f32 = 3.0;
const FADE_SECONDS: f32 = 0.6;

const TEXT: Color = Color::srgb(0.92, 0.96, 1.0);
const DIM: Color = Color::srgb(0.55, 0.62, 0.75);
const BUTTON: Color = Color::srgba(0.03, 0.05, 0.15, 0.8);
const SELECTED: Color = Color::srgba(0.1, 0.25, 0.45, 0.9);
const FRAME: Color = Color::srgb(0.3, 0.85, 1.0);
const FRAME_DIM: Color = Color::srgba(0.3, 0.85, 1.0, 0.25);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuSelection>()
            .init_resource::<FlightStarted>()
            .add_systems(OnEnter(AppState::Splash), spawn_splash)
            .add_systems(OnExit(AppState::Splash), despawn::<SplashScreen>)
            .add_systems(OnEnter(AppState::Menu), (spawn_menu, pause_world))
            .add_systems(OnExit(AppState::Menu), (despawn::<MenuScreen>, resume_world))
            .add_systems(OnEnter(AppState::Playing), |mut started: ResMut<FlightStarted>| started.0 = true)
            .add_systems(
                Update,
                (
                    (fallback_splash, run_splash).run_if(in_state(AppState::Splash)),
                    (navigate, hover_select, highlight).chain().run_if(in_state(AppState::Menu)),
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

#[derive(Component)]
struct MenuButton(usize);

#[derive(Resource, Default)]
struct MenuSelection(usize);

/// Ob schon geflogen wurde; dann heißt der erste Knopf „Weiterfliegen“.
#[derive(Resource, Default)]
struct FlightStarted(bool);

#[derive(Clone, Copy)]
enum Action {
    Fly,
    Quit,
}

const ACTIONS: [Action; 2] = [Action::Fly, Action::Quit];

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

fn spawn_menu(mut commands: Commands, assets: Res<AssetServer>, started: Res<FlightStarted>, mut selection: ResMut<MenuSelection>) {
    selection.0 = 0;
    let screen = commands
        .spawn((
            MenuScreen,
            fullscreen(),
            ImageNode { image: assets.load(MENU_IMAGE), image_mode: NodeImageMode::Stretch, ..default() },
            GlobalZIndex(40),
        ))
        .id();
    // Die Knöpfe sitzen in der freien Mitte des Bildes, unter dem Schriftzug.
    let column = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                top: Val::Percent(52.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            ChildOf(screen),
        ))
        .id();
    for (index, action) in ACTIONS.iter().enumerate() {
        let label = match action {
            Action::Fly if started.0 => "Weiterfliegen",
            Action::Fly => "Losfliegen",
            Action::Quit => "Beenden",
        };
        let button = commands
            .spawn((
                MenuButton(index),
                Hovered::default(),
                Node {
                    width: Val::Px(280.0),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BorderColor::from(FRAME_DIM),
                BackgroundColor(BUTTON),
                ChildOf(column),
            ))
            .id();
        commands.spawn((Text::new(label), TextColor(TEXT), font(24.0), ChildOf(button)));
        let action = *action;
        commands.entity(button).observe(move |click: On<PointerClick>, mut commands: Commands| {
            if click.button == PointerButton::Primary {
                commands.run_system_cached_with(activate, action);
            }
        });
    }
    commands.spawn((
        Text::new("W/S oder Pfeiltasten wählen · Enter bestätigen · im Flug: Esc für dieses Menü"),
        TextColor(DIM),
        font(15.0),
        Node {
            margin: UiRect::top(Val::Px(8.0)),
            padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BackgroundColor(BUTTON),
        ChildOf(column),
    ));
}

fn activate(action: In<Action>, mut next: ResMut<NextState<AppState>>, mut exit: MessageWriter<AppExit>) {
    match *action {
        Action::Fly => next.set(AppState::Playing),
        Action::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

fn navigate(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, mut selection: ResMut<MenuSelection>) {
    let pressed = |a: KeyCode, b: KeyCode| keys.just_pressed(a) || keys.just_pressed(b);
    let count = ACTIONS.len();
    if pressed(KeyCode::KeyW, KeyCode::ArrowUp) {
        selection.0 = (selection.0 + count - 1) % count;
    }
    if pressed(KeyCode::KeyS, KeyCode::ArrowDown) {
        selection.0 = (selection.0 + 1) % count;
    }
    if pressed(KeyCode::Enter, KeyCode::NumpadEnter) || keys.just_pressed(KeyCode::Space) {
        commands.run_system_cached_with(activate, ACTIONS[selection.0]);
    }
}

fn hover_select(buttons: Query<(&MenuButton, &Hovered), Changed<Hovered>>, mut selection: ResMut<MenuSelection>) {
    for (button, hovered) in &buttons {
        if hovered.get() {
            selection.0 = button.0;
        }
    }
}

fn highlight(selection: Res<MenuSelection>, mut buttons: Query<(&MenuButton, &mut BackgroundColor, &mut BorderColor)>) {
    for (button, mut background, mut border) in &mut buttons {
        let selected = button.0 == selection.0;
        background.0 = if selected { SELECTED } else { BUTTON };
        *border = BorderColor::from(if selected { FRAME } else { FRAME_DIM });
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
