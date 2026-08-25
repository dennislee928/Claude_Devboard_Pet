//! Persisted user settings (shared by both editions).

use crate::paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Config {
    /// "board" | "desktop" | "both"
    pub display: String,
    /// serial port override, e.g. COM9 or /dev/cu.usbserial-0001
    pub port: Option<String>,
    /// "clawd" | "beemo" | "grogu"
    pub character: String,
    /// pet slowly walks across the screen while idle
    pub wander: bool,
    /// status panel visible (never overlaps the pet — it docks beside it)
    pub panel: bool,
    /// which side the status panel docks to when there is room: "auto"|"left"|"right"
    pub panel_side: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            display: "both".into(),
            port: None,
            character: "clawd".into(),
            wander: false,
            panel: false,
            panel_side: "auto".into(),
        }
    }
}

pub fn path() -> PathBuf {
    paths::state_dir().join("config.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        paths::ensure_state_dir();
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path(), s);
        }
    }

    /// Character index into assets_gen::CHAR_NAMES (0..=2; 3 is the egg form).
    pub fn char_index(&self) -> usize {
        crate::assets_gen::CHAR_NAMES.iter().take(3).position(|n| *n == self.character).unwrap_or(0)
    }
}
