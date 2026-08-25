//! XP + level engine. Levels: 1 Egg, 2 Baby, 3 Junior, 4 Senior, 5 Legend.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const THRESHOLDS: [u64; 5] = [0, 100, 400, 1200, 3000];
pub const LEVEL_NAMES: [&str; 5] = ["Egg", "Baby", "Junior", "Senior", "Legend"];

#[derive(Serialize, Deserialize, Default)]
pub struct Growth {
    pub xp: u64,
}

impl Growth {
    pub fn level(&self) -> u8 {
        let mut lv = 1u8;
        for (i, &t) in THRESHOLDS.iter().enumerate() {
            if self.xp >= t {
                lv = (i + 1) as u8;
            }
        }
        lv
    }

    /// XP needed for the next level, None at max level.
    pub fn next_threshold(&self) -> Option<u64> {
        THRESHOLDS.iter().copied().find(|&t| t > self.xp)
    }

    /// Add XP; returns true when this crossed a level boundary.
    pub fn add(&mut self, amount: u64) -> bool {
        let before = self.level();
        self.xp += amount;
        self.level() > before
    }

    pub fn state_path() -> PathBuf {
        crate::paths::state_dir()
    }

    pub fn load() -> Self {
        let p = Self::state_path().join("pet_state.json");
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let dir = Self::state_path();
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(dir.join("pet_state.json"), s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds() {
        let mut g = Growth { xp: 0 };
        assert_eq!(g.level(), 1);
        assert!(!g.add(99));
        assert_eq!(g.level(), 1);
        assert!(g.add(1)); // 100 -> level 2
        assert_eq!(g.level(), 2);
        assert!(g.add(300)); // 400 -> level 3
        assert_eq!(g.level(), 3);
        g.xp = 2999;
        assert_eq!(g.level(), 4);
        assert!(g.add(1));
        assert_eq!(g.level(), 5);
        assert_eq!(g.next_threshold(), None);
    }
}
