//! Ein kleiner eigener Zufallsgenerator (xorshift64*): reicht fürs Spiel völlig und läuft ohne
//! Zusatz-Crates auch im Browser.

use bevy::prelude::*;

#[derive(Resource)]
pub struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        #[cfg(target_arch = "wasm32")]
        let seed = 0x9E37_79B9_7F4A_7C15;
        // Testläufe können mit NOVA_SEED immer denselben Ablauf erzeugen.
        let seed = std::env::var("NOVA_SEED").ok().and_then(|s| s.parse().ok()).filter(|_| cfg!(debug_assertions)).unwrap_or(seed);
        Self(seed | 1)
    }
}

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Gleichverteilt in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.f32()
    }

    /// Eine ganze Zahl in [0, n).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n.max(1) as u64) as usize
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Eine zufällige Richtung mit Länge 1.
    pub fn direction(&mut self) -> Vec2 {
        Vec2::from_angle(self.range(0.0, std::f32::consts::TAU))
    }
}
