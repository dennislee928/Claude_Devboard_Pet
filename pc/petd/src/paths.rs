//! Cross-platform locations for state, config and logs.
//!
//! Windows: %APPDATA%\devpet
//! macOS:   ~/Library/Application Support/devpet
//! Linux:   $XDG_CONFIG_HOME/devpet (or ~/.config/devpet)

use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Directory holding config.json, pet_state.json and petd.log.
pub fn state_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(home).join("devpet")
    } else if cfg!(target_os = "macos") {
        home().join("Library").join("Application Support").join("devpet")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home().join(".config"))
            .join("devpet")
    }
}

pub fn ensure_state_dir() -> PathBuf {
    let d = state_dir();
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn log_file() -> PathBuf {
    state_dir().join("petd.log")
}

/// Where Claude Code keeps its per-project session transcripts.
pub fn claude_projects_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".claude"))
        .join("projects")
}
