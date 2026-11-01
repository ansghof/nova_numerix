//! Ein kleiner Synthesizer: Alle Geräusche und Musikstücke werden beim Start im Code erzeugt
//! und als WAV an Bevy übergeben. So braucht das Spiel keine Tondateien, und der Klang passt zum
//! Neon-Stil: Rechteck, Dreieck, Sägezahn und Rauschen wie bei alten Spielkonsolen.

pub const RATE: u32 = 22050;

#[derive(Clone, Copy)]
pub enum Wave {
    Sine,
    Square,
    Triangle,
    Saw,
}

fn osc(wave: Wave, phase: f32) -> f32 {
    let p = phase.fract();
    match wave {
        Wave::Sine => (p * std::f32::consts::TAU).sin(),
        Wave::Square => {
            if p < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Wave::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * p - 1.0,
    }
}

/// Frequenz eines MIDI-Tons: 69 ist das eingestrichene A mit 440 Hz.
pub fn midi(note: i32) -> f32 {
    440.0 * 2f32.powf((note - 69) as f32 / 12.0)
}

/// Ein Tonpuffer, in den Klänge hineingemischt werden.
pub struct Buffer {
    pub samples: Vec<f32>,
    noise: u32,
}

impl Buffer {
    pub fn new(seconds: f32) -> Self {
        Self { samples: vec![0.0; (seconds * RATE as f32) as usize], noise: 0x1234_5678 }
    }

    fn white(&mut self) -> f32 {
        // Einfacher xorshift, reicht für Rauschen.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Ein Ton ab `start` Sekunden, `length` lang, der von `from` nach `to` Hz gleitet. Er setzt
    /// in `attack` Sekunden ein und klingt mit der Kurve `curve` aus (größer = kürzer).
    pub fn tone(&mut self, start: f32, length: f32, from: f32, to: f32, wave: Wave, volume: f32, attack: f32, curve: f32) {
        let first = (start * RATE as f32) as usize;
        let count = (length * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..count {
            let Some(sample) = self.samples.get_mut(first + i) else {
                break;
            };
            let t = i as f32 / count as f32;
            let freq = from * (to / from).powf(t);
            phase += freq / RATE as f32;
            let seconds = i as f32 / RATE as f32;
            let envelope = (seconds / attack.max(0.0005)).min(1.0) * (1.0 - t).powf(curve);
            *sample += osc(wave, phase) * volume * envelope;
        }
    }

    /// Rauschen; `brightness` von 0 (dumpf) bis 1 (zischend), negativ für nur hohe Anteile.
    pub fn noise(&mut self, start: f32, length: f32, volume: f32, brightness: f32, curve: f32) {
        let first = (start * RATE as f32) as usize;
        let count = (length * RATE as f32) as usize;
        let alpha = brightness.abs().clamp(0.01, 1.0);
        let mut low = 0.0f32;
        for i in 0..count {
            if first + i >= self.samples.len() {
                break;
            }
            let white = self.white();
            low += alpha * (white - low);
            let value = if brightness < 0.0 { white - low } else { low };
            let t = i as f32 / count as f32;
            self.samples[first + i] += value * volume * (1.0 - t).powf(curve);
        }
    }

    /// Ein einfaches Echo, das den Klang weicher und größer macht.
    pub fn echo(&mut self, delay: f32, feedback: f32) {
        let offset = (delay * RATE as f32) as usize;
        for i in offset..self.samples.len() {
            self.samples[i] += self.samples[i - offset] * feedback;
        }
    }

    /// Als 16-Bit-Mono-WAV, mit sanfter Begrenzung gegen Übersteuern.
    pub fn wav(&self, gain: f32) -> Vec<u8> {
        let data_len = (self.samples.len() * 2) as u32;
        let mut out = Vec::with_capacity(44 + data_len as usize);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&RATE.to_le_bytes());
        out.extend_from_slice(&(RATE * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in &self.samples {
            let value = (sample * gain).tanh() * 0.95;
            out.extend_from_slice(&((value * i16::MAX as f32) as i16).to_le_bytes());
        }
        out
    }
}

/// Die Geräusche des Spiels.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Sfx {
    Laser,
    Missile,
    Explosion,
    BigExplosion,
    PlayerHit,
    ShipLost,
    TaskOpen,
    Correct,
    Fast,
    Wrong,
    Timeout,
    Super,
    WaveCleared,
    Checkpoint,
    Click,
    Buy,
    BossAlarm,
    Victory,
    Plasma,
    Zap,
}

impl Sfx {
    pub const ALL: [Sfx; 20] = [
        Sfx::Laser,
        Sfx::Missile,
        Sfx::Explosion,
        Sfx::BigExplosion,
        Sfx::PlayerHit,
        Sfx::ShipLost,
        Sfx::TaskOpen,
        Sfx::Correct,
        Sfx::Fast,
        Sfx::Wrong,
        Sfx::Timeout,
        Sfx::Super,
        Sfx::WaveCleared,
        Sfx::Checkpoint,
        Sfx::Click,
        Sfx::Buy,
        Sfx::BossAlarm,
        Sfx::Victory,
        Sfx::Plasma,
        Sfx::Zap,
    ];

    /// Wie laut das Geräusch im Mix ist; Dauerfeuer bleibt leise.
    pub fn volume(self) -> f32 {
        match self {
            Sfx::Laser => 0.25,
            Sfx::Missile => 0.35,
            Sfx::Explosion => 0.5,
            Sfx::Click => 0.4,
            Sfx::Plasma | Sfx::Zap => 0.35,
            _ => 0.7,
        }
    }

    /// So viele Sekunden muss mindestens zwischen zwei gleichen Geräuschen liegen.
    pub fn spacing(self) -> f32 {
        match self {
            Sfx::Laser => 0.11,
            Sfx::Missile => 0.2,
            Sfx::Explosion => 0.06,
            Sfx::PlayerHit => 0.15,
            Sfx::Zap => 0.1,
            _ => 0.0,
        }
    }

    pub fn render(self) -> Vec<u8> {
        let mut b;
        match self {
            Sfx::Laser => {
                b = Buffer::new(0.09);
                b.tone(0.0, 0.08, 1500.0, 500.0, Wave::Square, 0.35, 0.002, 2.0);
            }
            Sfx::Missile => {
                b = Buffer::new(0.3);
                b.tone(0.0, 0.28, 250.0, 900.0, Wave::Saw, 0.2, 0.01, 1.5);
                b.noise(0.0, 0.28, 0.35, 0.3, 1.5);
            }
            Sfx::Explosion => {
                b = Buffer::new(0.45);
                b.noise(0.0, 0.42, 0.9, 0.25, 2.5);
                b.tone(0.0, 0.3, 110.0, 40.0, Wave::Sine, 0.7, 0.002, 2.0);
            }
            Sfx::BigExplosion => {
                b = Buffer::new(1.8);
                b.noise(0.0, 1.7, 1.0, 0.12, 2.0);
                b.tone(0.0, 1.2, 90.0, 28.0, Wave::Sine, 0.9, 0.002, 1.5);
                b.noise(0.15, 0.6, 0.5, 0.5, 3.0);
            }
            Sfx::PlayerHit => {
                b = Buffer::new(0.2);
                b.tone(0.0, 0.18, 260.0, 110.0, Wave::Square, 0.35, 0.002, 1.5);
                b.noise(0.0, 0.1, 0.4, 0.4, 2.0);
            }
            Sfx::ShipLost => {
                b = Buffer::new(1.2);
                b.tone(0.0, 1.0, 500.0, 50.0, Wave::Saw, 0.35, 0.005, 1.2);
                b.noise(0.0, 1.1, 0.8, 0.15, 1.8);
            }
            Sfx::TaskOpen => {
                b = Buffer::new(0.35);
                b.tone(0.0, 0.12, midi(76), midi(76), Wave::Triangle, 0.5, 0.003, 1.5);
                b.tone(0.08, 0.22, midi(81), midi(81), Wave::Triangle, 0.5, 0.003, 1.5);
            }
            Sfx::Correct => {
                b = Buffer::new(0.5);
                for (i, note) in [72, 76, 79].into_iter().enumerate() {
                    let length = if i == 2 { 0.3 } else { 0.1 };
                    b.tone(i as f32 * 0.07, length, midi(note), midi(note), Wave::Triangle, 0.5, 0.003, 1.5);
                }
            }
            Sfx::Fast => {
                b = Buffer::new(0.7);
                for (i, note) in [72, 76, 79, 84].into_iter().enumerate() {
                    let length = if i == 3 { 0.35 } else { 0.09 };
                    b.tone(i as f32 * 0.06, length, midi(note), midi(note), Wave::Square, 0.22, 0.003, 1.5);
                    b.tone(i as f32 * 0.06, length, midi(note + 12), midi(note + 12), Wave::Sine, 0.2, 0.003, 2.0);
                }
                b.echo(0.11, 0.3);
            }
            // Bewusst sanft: ein Fehler soll nicht wehtun.
            Sfx::Wrong => {
                b = Buffer::new(0.45);
                b.tone(0.0, 0.16, midi(55), midi(55), Wave::Triangle, 0.5, 0.005, 1.5);
                b.tone(0.15, 0.25, midi(52), midi(51), Wave::Triangle, 0.5, 0.005, 1.5);
            }
            Sfx::Timeout => {
                b = Buffer::new(0.45);
                b.tone(0.0, 0.16, midi(69), midi(69), Wave::Triangle, 0.4, 0.005, 1.5);
                b.tone(0.15, 0.25, midi(64), midi(64), Wave::Triangle, 0.4, 0.005, 1.5);
            }
            Sfx::Super => {
                b = Buffer::new(1.4);
                b.tone(0.0, 0.5, 120.0, 1400.0, Wave::Saw, 0.3, 0.01, 0.5);
                b.noise(0.35, 0.9, 0.9, 0.2, 2.0);
                b.tone(0.4, 0.9, 80.0, 30.0, Wave::Sine, 0.9, 0.002, 1.5);
            }
            Sfx::WaveCleared => {
                b = Buffer::new(0.9);
                for (i, note) in [79, 83, 86, 91, 95].into_iter().enumerate() {
                    b.tone(i as f32 * 0.045, 0.2, midi(note), midi(note), Wave::Sine, 0.35, 0.002, 2.0);
                }
                b.echo(0.09, 0.35);
            }
            Sfx::Checkpoint => {
                b = Buffer::new(0.9);
                for (i, note) in [67, 72, 76].into_iter().enumerate() {
                    let start = i as f32 * 0.12;
                    b.tone(start, 0.5, midi(note), midi(note), Wave::Sine, 0.4, 0.003, 2.0);
                    b.tone(start, 0.3, midi(note + 12), midi(note + 12), Wave::Sine, 0.15, 0.003, 3.0);
                }
            }
            Sfx::Click => {
                b = Buffer::new(0.04);
                b.tone(0.0, 0.03, 1800.0, 1200.0, Wave::Square, 0.25, 0.001, 3.0);
            }
            Sfx::Buy => {
                b = Buffer::new(0.45);
                b.tone(0.0, 0.07, midi(83), midi(83), Wave::Square, 0.25, 0.002, 1.0);
                b.tone(0.07, 0.35, midi(88), midi(88), Wave::Square, 0.25, 0.002, 2.0);
            }
            Sfx::BossAlarm => {
                b = Buffer::new(1.5);
                for i in 0..3 {
                    let start = i as f32 * 0.45;
                    b.tone(start, 0.22, 620.0, 620.0, Wave::Square, 0.2, 0.01, 0.5);
                    b.tone(start + 0.22, 0.22, 460.0, 460.0, Wave::Square, 0.2, 0.01, 0.5);
                }
            }
            Sfx::Plasma => {
                b = Buffer::new(0.4);
                b.tone(0.0, 0.35, 180.0, 90.0, Wave::Saw, 0.3, 0.01, 1.2);
                b.tone(0.0, 0.3, 720.0, 360.0, Wave::Sine, 0.25, 0.01, 1.5);
            }
            Sfx::Zap => {
                b = Buffer::new(0.2);
                b.noise(0.0, 0.18, 0.5, -0.6, 2.0);
                b.tone(0.0, 0.15, 2400.0, 900.0, Wave::Square, 0.15, 0.001, 2.0);
            }
            Sfx::Victory => {
                b = Buffer::new(2.2);
                for (i, note) in [72, 76, 79].into_iter().enumerate() {
                    b.tone(i as f32 * 0.14, 0.14, midi(note), midi(note), Wave::Square, 0.2, 0.003, 1.0);
                }
                for note in [72, 76, 79, 84] {
                    b.tone(0.42, 1.6, midi(note), midi(note), Wave::Triangle, 0.22, 0.01, 1.2);
                }
                b.echo(0.15, 0.25);
            }
        }
        b.wav(1.2)
    }
}

/// Ein Musikstück: Tempo, Akkordfolge (je Takt ein Akkord) und Klangfarben.
pub struct Song {
    pub bpm: f32,
    /// Akkorde als MIDI-Töne (Grundton, Terz, Quinte).
    pub chords: &'static [[i32; 3]],
    pub bass: Wave,
    pub arp: Wave,
    /// Welche Akkordtöne das Arpeggio der Reihe nach spielt.
    pub pattern: &'static [usize],
    /// Bass in Sechzehnteln statt Achteln.
    pub driving: bool,
    pub drums: bool,
    /// Weiche Flächen statt Schlagzeug, für Menü und Werft.
    pub pad: bool,
}

/// Menü und Werft: ruhig, ohne Schlagzeug.
pub const MENU: Song = Song {
    bpm: 92.0,
    chords: &[[60, 64, 67], [57, 60, 64], [53, 57, 60], [55, 59, 62]],
    bass: Wave::Triangle,
    arp: Wave::Sine,
    pattern: &[0, 1, 2, 1],
    driving: false,
    drums: false,
    pad: true,
};

/// Bosskampf: schneller, treibender Bass.
pub const BOSS: Song = Song {
    bpm: 140.0,
    chords: &[[57, 60, 64], [57, 60, 64], [58, 62, 65], [56, 59, 64]],
    bass: Wave::Saw,
    arp: Wave::Square,
    pattern: &[0, 2, 1, 2],
    driving: true,
    drums: true,
    pad: false,
};

/// Je Level ein eigenes Stück.
pub const LEVEL_SONGS: [Song; 5] = [
    // Neonnebel: a-Moll, entspannt treibend
    Song {
        bpm: 118.0,
        chords: &[[57, 60, 64], [53, 57, 60], [60, 64, 67], [55, 59, 62]],
        bass: Wave::Saw,
        arp: Wave::Triangle,
        pattern: &[0, 1, 2, 1],
        driving: false,
        drums: true,
        pad: false,
    },
    // Gitterstation: d-Moll, kantiger
    Song {
        bpm: 124.0,
        chords: &[[62, 65, 69], [58, 62, 65], [55, 58, 62], [57, 61, 64]],
        bass: Wave::Square,
        arp: Wave::Square,
        pattern: &[0, 2, 1, 2],
        driving: false,
        drums: true,
        pad: false,
    },
    // Gasriese: e-Moll, schwer und langsam
    Song {
        bpm: 108.0,
        chords: &[[52, 55, 59], [48, 52, 55], [57, 60, 64], [59, 63, 66]],
        bass: Wave::Saw,
        arp: Wave::Triangle,
        pattern: &[0, 1, 2, 0],
        driving: true,
        drums: true,
        pad: false,
    },
    // Datenkern: cis-Moll, schnell und digital
    Song {
        bpm: 132.0,
        chords: &[[61, 64, 68], [57, 61, 64], [64, 68, 71], [59, 63, 66]],
        bass: Wave::Square,
        arp: Wave::Square,
        pattern: &[0, 1, 2, 1, 2, 0],
        driving: true,
        drums: true,
        pad: false,
    },
    // Mars-Außenposten: g-Moll, ein Marsch
    Song {
        bpm: 112.0,
        chords: &[[55, 58, 62], [51, 55, 58], [53, 57, 60], [50, 54, 57]],
        bass: Wave::Square,
        arp: Wave::Saw,
        pattern: &[0, 2, 0, 1],
        driving: false,
        drums: true,
        pad: false,
    },
];

impl Song {
    /// 16 Takte: erst Bass und Schlagzeug, dann kommt das Arpeggio dazu, zuletzt eine Oktave höher.
    pub fn render(&self) -> Vec<u8> {
        let step = 60.0 / self.bpm / 4.0;
        let bar = step * 16.0;
        let bars = 16;
        let mut b = Buffer::new(bar * bars as f32);
        for bar_index in 0..bars {
            let chord = self.chords[bar_index % self.chords.len()];
            let section = bar_index / 4;
            let start = bar_index as f32 * bar;
            // Bass
            let bass_step = if self.driving { 1 } else { 2 };
            for s in (0..16).step_by(bass_step) {
                let note = chord[0] - 24 + if s % 8 == 6 && !self.driving { 12 } else { 0 };
                let at = start + s as f32 * step;
                b.tone(at, step * bass_step as f32 * 0.9, midi(note), midi(note), self.bass, 0.22, 0.004, 1.2);
            }
            // Flächen im Menü
            if self.pad {
                for note in chord {
                    b.tone(start, bar, midi(note - 12), midi(note - 12), Wave::Triangle, 0.07, 0.4, 0.6);
                    b.tone(start, bar, midi(note), midi(note) * 1.003, Wave::Sine, 0.05, 0.6, 0.8);
                }
            }
            // Arpeggio ab dem zweiten Abschnitt, im letzten eine Oktave höher
            if section >= 1 {
                let octave = if section == 3 { 24 } else { 12 };
                let every = if self.pad { 2 } else { 1 };
                for s in (0..16).step_by(every) {
                    let note = chord[self.pattern[(s / every) % self.pattern.len()]] + octave;
                    let at = start + s as f32 * step;
                    let volume = if self.pad { 0.08 } else { 0.1 };
                    b.tone(at, step * every as f32 * 0.9, midi(note), midi(note), self.arp, volume, 0.003, 2.0);
                }
            }
            // Schlagzeug
            if self.drums {
                let kicks: &[usize] = if self.driving { &[0, 4, 8, 12] } else { &[0, 8, 10] };
                for &s in kicks {
                    b.tone(start + s as f32 * step, 0.18, 150.0, 45.0, Wave::Sine, 0.55, 0.001, 2.0);
                }
                for s in [4, 12] {
                    let at = start + s as f32 * step;
                    b.noise(at, 0.14, 0.3, 0.6, 2.5);
                    b.tone(at, 0.08, 200.0, 160.0, Wave::Triangle, 0.15, 0.001, 2.0);
                }
                for s in (0..16).step_by(2) {
                    b.noise(start + s as f32 * step, 0.03, 0.12, -0.3, 2.0);
                }
            }
        }
        b.wav(0.9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schreibt alle Klänge nach `target/sounds`, zum Anhören.
    #[test]
    fn klaenge_ausgeben() {
        let dir = std::path::Path::new("target").join("sounds");
        std::fs::create_dir_all(&dir).unwrap();
        for sfx in Sfx::ALL {
            std::fs::write(dir.join(format!("{sfx:?}.wav")), sfx.render()).unwrap();
        }
        std::fs::write(dir.join("Musik-Menue.wav"), MENU.render()).unwrap();
        std::fs::write(dir.join("Musik-Boss.wav"), BOSS.render()).unwrap();
        for (i, song) in LEVEL_SONGS.iter().enumerate() {
            std::fs::write(dir.join(format!("Musik-Level{}.wav", i + 1)), song.render()).unwrap();
        }
    }

    #[test]
    fn wav_ist_gueltig() {
        for sfx in Sfx::ALL {
            let wav = sfx.render();
            assert_eq!(&wav[0..4], b"RIFF", "{sfx:?}");
            assert!(wav.len() > 44 + 100, "{sfx:?}");
        }
        let song = MENU.render();
        let seconds = (song.len() - 44) as f32 / 2.0 / RATE as f32;
        assert!((40.0..44.0).contains(&seconds), "{seconds}");
    }
}
