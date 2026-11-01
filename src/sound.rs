//! Geräusche und Musik. Alles wird beim Start mit [`crate::synth`] erzeugt: die Geräusche sofort,
//! die Musikstücke in einem eigenen Thread, damit der Start nicht wartet.
//!
//! Andere Module lösen ein Geräusch mit [`play`] aus. Die Musik folgt dem Spielzustand: ruhig in
//! Menü und Werft, je Level ein eigenes Stück, im Bosskampf ein schnelleres; Wechsel werden
//! überblendet. Während einer Aufgabe wird die Musik leiser, damit man sich aufs Rechnen
//! konzentrieren kann. M schaltet die Musik, N die Geräusche an und aus.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use bevy::prelude::*;

use crate::boss::{Boss, BossState};
use crate::enemy::WaveCleared;
use crate::level::{CheckpointReached, CurrentLevel, LevelStats, Rewind};
use crate::player::Player;
use crate::superweapon::FireSuper;
use crate::task::FAST_SHARE;
use crate::synth::{BOSS, LEVEL_SONGS, MENU, Sfx, Song};
use crate::{AppState, GamePhase};

/// Lautstärke der Musik, im Hintergrund und während einer Aufgabe.
const MUSIC_VOLUME: f32 = 0.45;
const MUSIC_DUCKED: f32 = 0.15;
/// Wie schnell die Musik ein- und ausblendet (Lautstärke je Sekunde).
const FADE_SPEED: f32 = 0.6;
const SETTINGS_SHOWN: f32 = 2.0;

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .init_resource::<AudioSettings>()
            .add_systems(Startup, (build_sounds, spawn_settings_label))
            .add_systems(OnEnter(GamePhase::Task), |mut commands: Commands| play(&mut commands, Sfx::TaskOpen))
            .add_systems(
                Update,
                (receive_music, sounds_from_messages, sounds_from_state, play_sfx, choose_music, toggle, show_settings).chain(),
            );
    }
}

/// Ein Geräusch abspielen.
#[derive(Message)]
pub struct PlaySfx(pub Sfx);

/// Bequem aus jedem System mit `Commands`.
pub fn play(commands: &mut Commands, sfx: Sfx) {
    commands.write_message(PlaySfx(sfx));
}

#[derive(Resource)]
pub struct AudioSettings {
    pub music: bool,
    pub sfx: bool,
}

impl Default for AudioSettings {
    /// Automatische Testläufe sind stumm, außer `NOVA_TON` ist gesetzt.
    fn default() -> Self {
        let on = !(cfg!(debug_assertions) && std::env::var("NOVA_TEST").is_ok() && std::env::var("NOVA_TON").is_err());
        Self { music: on, sfx: on }
    }
}

/// Welches Musikstück: Menü, ein Level oder der Boss.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Track {
    Menu,
    Level(usize),
    Boss,
}

#[derive(Resource)]
struct SoundBank {
    sfx: HashMap<Sfx, Handle<AudioSource>>,
    music: HashMap<Track, Handle<AudioSource>>,
    /// Die Musik entsteht im Hintergrund und kommt Stück für Stück hier an.
    incoming: Mutex<Receiver<(Track, Vec<u8>)>>,
    /// Im Browser gibt es keine Threads: Dort wird je Frame ein Stück erzeugt.
    pending: Vec<(Track, &'static Song)>,
}

/// Erst das Menü, das zuerst gebraucht wird, dann die Level und der Boss.
fn songs() -> Vec<(Track, &'static Song)> {
    let mut songs = vec![(Track::Menu, &MENU)];
    songs.extend(LEVEL_SONGS.iter().enumerate().map(|(i, song)| (Track::Level(i), song)));
    songs.push((Track::Boss, &BOSS));
    songs
}

#[derive(Component)]
struct Music {
    track: Track,
    volume: f32,
}

#[derive(Component)]
struct SettingsLabel;

fn source(bytes: Vec<u8>) -> AudioSource {
    AudioSource { bytes: bytes.into() }
}

fn build_sounds(mut commands: Commands, mut sources: ResMut<Assets<AudioSource>>) {
    let sfx = Sfx::ALL.iter().map(|s| (*s, sources.add(source(s.render())))).collect();
    let (sender, receiver) = channel();
    let pending = if cfg!(target_arch = "wasm32") {
        songs()
    } else {
        std::thread::spawn(move || {
            for (track, song) in songs() {
                if sender.send((track, song.render())).is_err() {
                    return;
                }
            }
        });
        Vec::new()
    };
    commands.insert_resource(SoundBank { sfx, music: HashMap::new(), incoming: Mutex::new(receiver), pending });
}

fn receive_music(mut bank: ResMut<SoundBank>, mut sources: ResMut<Assets<AudioSource>>) {
    let ready: Vec<_> = match bank.incoming.lock() {
        Ok(receiver) => receiver.try_iter().collect(),
        Err(_) => return,
    };
    for (track, bytes) in ready {
        let handle = sources.add(source(bytes));
        bank.music.insert(track, handle);
    }
    if !bank.pending.is_empty() {
        let (track, song) = bank.pending.remove(0);
        let handle = sources.add(source(song.render()));
        bank.music.insert(track, handle);
    }
}

fn play_sfx(
    mut commands: Commands,
    mut requests: MessageReader<PlaySfx>,
    bank: Option<Res<SoundBank>>,
    settings: Res<AudioSettings>,
    time: Res<Time<Real>>,
    mut last: Local<HashMap<Sfx, f32>>,
) {
    let Some(bank) = bank else {
        requests.clear();
        return;
    };
    let now = time.elapsed_secs();
    for PlaySfx(sfx) in requests.read() {
        if !settings.sfx {
            continue;
        }
        // Dauerfeuer und Explosionsketten nicht zu einem Brei werden lassen.
        if last.get(sfx).is_some_and(|at| now - at < sfx.spacing()) {
            continue;
        }
        last.insert(*sfx, now);
        if let Some(handle) = bank.sfx.get(sfx) {
            commands.spawn((AudioPlayer(handle.clone()), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(sfx.volume()))));
        }
    }
}

/// Geräusche zu Ereignissen, die ohnehin als Nachricht kommen.
fn sounds_from_messages(
    mut commands: Commands,
    mut checkpoints: MessageReader<CheckpointReached>,
    mut waves: MessageReader<WaveCleared>,
    mut supers: MessageReader<FireSuper>,
    mut rewinds: MessageReader<Rewind>,
) {
    if checkpoints.read().count() > 0 {
        play(&mut commands, Sfx::Checkpoint);
    }
    if waves.read().count() > 0 {
        play(&mut commands, Sfx::WaveCleared);
    }
    if supers.read().count() > 0 {
        play(&mut commands, Sfx::Super);
    }
    if rewinds.read().count() > 0 {
        play(&mut commands, Sfx::ShipLost);
    }
}

/// Was sich am Spielstand ablesen lässt: Aufgaben-Ergebnisse (auch am Boss), Treffer am Schiff,
/// Auftritt und Untergang des Bosses.
fn sounds_from_state(
    mut commands: Commands,
    time: Res<Time<Real>>,
    stats: Res<LevelStats>,
    phase: Res<State<GamePhase>>,
    boss_state: Res<BossState>,
    new_boss: Query<(), Added<Boss>>,
    player: Single<&Player>,
    mut last_stats: Local<[u32; 4]>,
    mut last_health: Local<Option<f32>>,
    mut was_defeated: Local<bool>,
    mut victory_in: Local<Option<f32>>,
) {
    let now = [stats.correct, stats.fast, stats.timed_out, stats.wrong];
    if stats.is_changed() {
        let more = |i: usize| now[i] > last_stats[i];
        if more(1) {
            play(&mut commands, Sfx::Fast);
        } else if more(0) {
            play(&mut commands, Sfx::Correct);
        } else if more(3) {
            play(&mut commands, Sfx::Wrong);
        } else if more(2) {
            play(&mut commands, Sfx::Timeout);
        }
    }
    *last_stats = now;

    // Treffer nur im Flug; der kleine Abzug bei falschen Antworten hat sein eigenes Geräusch.
    if let Some(before) = *last_health
        && player.health < before - 0.5
        && *phase.get() == GamePhase::Flying
        && player.health > 0.0
    {
        play(&mut commands, Sfx::PlayerHit);
    }
    *last_health = Some(player.health);

    if !new_boss.is_empty() {
        play(&mut commands, Sfx::BossAlarm);
    }
    if boss_state.defeated && !*was_defeated {
        play(&mut commands, Sfx::BigExplosion);
        *victory_in = Some(1.0);
    }
    *was_defeated = boss_state.defeated;
    if let Some(left) = &mut *victory_in {
        *left -= time.delta_secs();
        if *left <= 0.0 {
            *victory_in = None;
            play(&mut commands, Sfx::Victory);
        }
    }
}

/// Für die Werft: schnell oder normal richtig, falsch oder zu langsam.
pub fn outcome_sfx(outcome: &crate::task::Outcome) -> Sfx {
    match outcome {
        crate::task::Outcome::Correct { remaining_share } if *remaining_share >= FAST_SHARE => Sfx::Fast,
        crate::task::Outcome::Correct { .. } => Sfx::Correct,
        crate::task::Outcome::TimedOut => Sfx::Timeout,
        crate::task::Outcome::Wrong => Sfx::Wrong,
    }
}

fn wanted_track(state: &AppState, current: &CurrentLevel, boss: Option<&Boss>, defeated: bool) -> Option<Track> {
    match state {
        AppState::Splash | AppState::Menu | AppState::Shipyard => Some(Track::Menu),
        AppState::Playing if defeated => None,
        // Solange der Boss da ist, Bossmusik; während er untergeht, Stille für den großen Knall.
        AppState::Playing => match boss {
            Some(boss) if boss.alive() => Some(Track::Boss),
            Some(_) => None,
            None => Some(Track::Level(current.0)),
        },
    }
}

/// Spielt das passende Stück und blendet zwischen Stücken über.
fn choose_music(
    mut commands: Commands,
    time: Res<Time<Real>>,
    bank: Option<Res<SoundBank>>,
    settings: Res<AudioSettings>,
    state: Res<State<AppState>>,
    phase: Res<State<GamePhase>>,
    current: Res<CurrentLevel>,
    boss_state: Res<BossState>,
    boss: Query<&Boss>,
    mut playing: Query<(Entity, &mut Music, Option<&mut AudioSink>)>,
) {
    let Some(bank) = bank else {
        return;
    };
    let wanted = wanted_track(state.get(), &current, boss.iter().next(), boss_state.defeated);
    let in_task = *state.get() == AppState::Playing && *phase.get() != GamePhase::Flying;
    let loud = if !settings.music {
        0.0
    } else if in_task {
        MUSIC_DUCKED
    } else {
        MUSIC_VOLUME
    };
    let step = FADE_SPEED * time.delta_secs();
    let mut have_wanted = false;
    for (entity, mut music, sink) in &mut playing {
        let target = if Some(music.track) == wanted { loud } else { 0.0 };
        have_wanted |= Some(music.track) == wanted;
        music.volume += (target - music.volume).clamp(-step, step);
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(music.volume));
        }
        if Some(music.track) != wanted && music.volume <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
    if let Some(track) = wanted
        && !have_wanted
        && let Some(handle) = bank.music.get(&track)
    {
        info!("Musik: {track:?}");
        commands.spawn((
            Music { track, volume: 0.0 },
            AudioPlayer(handle.clone()),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
}

/// M: Musik an/aus, N: Geräusche an/aus. Nicht im Menü, dort wird der Schiffsname getippt.
fn toggle(keys: Res<ButtonInput<KeyCode>>, state: Res<State<AppState>>, mut settings: ResMut<AudioSettings>) {
    if matches!(state.get(), AppState::Menu | AppState::Splash) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyM) {
        settings.music = !settings.music;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        settings.sfx = !settings.sfx;
    }
}

fn spawn_settings_label(mut commands: Commands) {
    commands.spawn((
        SettingsLabel,
        Text::new(""),
        TextFont {
            font: crate::fonts::family(),
            ..TextFont::from_font_size(15.0)
        },
        TextColor(Color::srgb(0.8, 0.9, 1.0)),
        Node { position_type: PositionType::Absolute, right: Val::Px(16.0), bottom: Val::Px(12.0), ..default() },
        GlobalZIndex(60),
    ));
}

/// Nach einem Umschalten steht kurz unten rechts, was an und aus ist.
fn show_settings(
    time: Res<Time<Real>>,
    settings: Res<AudioSettings>,
    mut label: Single<&mut Text, With<SettingsLabel>>,
    mut shown: Local<f32>,
) {
    if settings.is_changed() && !settings.is_added() {
        let state = |on: bool| if on { "an" } else { "aus" };
        label.0 = format!("Musik (M): {} · Geräusche (N): {}", state(settings.music), state(settings.sfx));
        *shown = SETTINGS_SHOWN;
    }
    if *shown > 0.0 {
        *shown -= time.delta_secs();
        if *shown <= 0.0 {
            label.0.clear();
        }
    }
}
