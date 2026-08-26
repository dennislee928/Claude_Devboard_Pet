//! Persisted user settings (shared by both editions).

use crate::paths;
use crate::usage::Budgets;
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
    /// Coding agents to watch: any of "claude", "codex".
    pub providers: Vec<String>,
    /// Which one drives the pet's state and the board screen.
    pub primary_provider: String,
    /// Token budgets, so estimated usage percentages mean something. Anything
    /// left at zero is shown as tokens without a percentage rather than
    /// measured against a limit we made up.
    pub budgets: Budgets,
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
            providers: vec!["claude".into()],
            primary_provider: "claude".into(),
            budgets: Budgets::default(),
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

    /// Enabled providers, always with the primary one included so the pet
    /// cannot end up driven by something it is not watching.
    pub fn enabled_providers(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .providers
            .iter()
            .filter(|p| crate::providers::ALL.iter().any(|a| a == p))
            .cloned()
            .collect();
        let primary_known = crate::providers::ALL.iter().any(|a| *a == self.primary_provider);
        if !v.contains(&self.primary_provider) && primary_known {
            v.push(self.primary_provider.clone());
        }
        if v.is_empty() {
            v.push(crate::providers::CLAUDE.into());
        }
        v.dedup();
        v
    }

    pub fn primary(&self) -> String {
        if crate::providers::ALL.contains(&self.primary_provider.as_str()) {
            self.primary_provider.clone()
        } else {
            crate::providers::CLAUDE.into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_primary_provider_is_always_watched() {
        let cfg = Config { providers: vec!["claude".into()], primary_provider: "codex".into(), ..Default::default() };
        assert_eq!(cfg.enabled_providers(), vec!["claude".to_string(), "codex".to_string()]);
        assert_eq!(cfg.primary(), "codex");
    }

    #[test]
    fn unknown_providers_are_dropped() {
        let cfg = Config { providers: vec!["gemini".into()], primary_provider: "gemini".into(), ..Default::default() };
        assert_eq!(cfg.enabled_providers(), vec!["claude".to_string()]);
        assert_eq!(cfg.primary(), "claude");
    }
}
