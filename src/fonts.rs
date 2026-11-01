//! Die Schrift des Spiels. Auf dem PC kommt sie aus dem System (Segoe UI hat alle Umlaute). Im
//! Browser gibt es keine Systemschriften, dort wird Fira Sans aus `assets/fonts` mitgeliefert
//! (SIL Open Font License, siehe `assets/fonts/FiraSans-License.txt`).

use bevy::prelude::*;

const SYSTEM_FAMILIES: &str = "'Segoe UI', 'Noto Sans', sans-serif";

pub struct FontsPlugin;

impl Plugin for FontsPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(target_arch = "wasm32")]
        _app.add_systems(PreStartup, web::load);
    }
}

/// Die Schriftquelle für alle Texte.
pub fn family() -> FontSource {
    #[cfg(target_arch = "wasm32")]
    if let Some(handle) = web::FIRA.get() {
        return FontSource::Handle(handle.clone());
    }
    FontSource::families(SYSTEM_FAMILIES)
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::sync::OnceLock;

    use bevy::prelude::*;

    pub static FIRA: OnceLock<Handle<Font>> = OnceLock::new();

    pub fn load(assets: Res<AssetServer>) {
        let _ = FIRA.set(assets.load("fonts/FiraSans-Regular.ttf"));
    }
}
