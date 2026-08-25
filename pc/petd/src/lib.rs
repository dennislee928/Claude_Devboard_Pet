//! DevPet — shared engine behind both editions of the app.
//!
//! * **Standalone edition** (`petd`) — everything runs on the PC. The dev board
//!   is optional and only mirrors the pet.
//! * **Firmware edition** (`petd-lite`) — the board is the brain: it runs the
//!   state machine, the growth engine and the animation. The PC binary is a
//!   thin bridge that forwards Claude Code hook events over USB and can
//!   *optionally* draw the same pet on the PC screen, with no install needed.
//!
//! Both editions share this crate: sprites, hook parsing, session tracking and
//! the desktop window.

pub mod assets_gen;
pub mod autostart;
pub mod board;
pub mod config;
pub mod daemon;
pub mod desktop;
pub mod growth;
pub mod paths;
pub mod server;
pub mod sessions;
pub mod state;

use std::sync::{Arc, Mutex};

/// Everything the UI thread needs, written by the dispatcher thread.
pub struct Shared {
    pub state: usize,
    /// index into assets_gen::CHAR_NAMES (0 clawd, 1 beemo, 2 grogu)
    pub chr: usize,
    pub level: u8,
    pub xp: u64,
    pub next: Option<u64>,
    pub board_connected: bool,
    pub board_enabled: bool,
    pub wander: bool,
    pub panel: bool,
    pub panel_side: String,
    /// "standalone" or "firmware" — shown in the status panel.
    pub backend: &'static str,
    pub sessions: sessions::Snapshot,
}

impl Shared {
    pub fn new(cfg: &config::Config, backend: &'static str, level: u8, xp: u64, next: Option<u64>) -> Arc<Mutex<Shared>> {
        Arc::new(Mutex::new(Shared {
            state: state::IDLE,
            chr: cfg.char_index(),
            level,
            xp,
            next,
            board_connected: false,
            board_enabled: cfg.display != "desktop",
            wander: cfg.wander,
            panel: cfg.panel,
            panel_side: cfg.panel_side.clone(),
            backend,
            sessions: sessions::Snapshot::default(),
        }))
    }
}

/// Flags both binaries understand. Returns None when the process should exit
/// (help printed, autostart installed, daemon launcher finished).
pub struct Cli {
    pub cfg: config::Config,
    pub reset_growth: bool,
}

pub const USAGE: &str = "\
petd — DevPet desk pet

  --display board|desktop|both   where the pet is shown (default both)
  --port <PORT>                  serial port (COM9, /dev/cu.usbserial-…)
  --char clawd|beemo|grogu       which pet
  --panel                        open the Claude Code status panel
  --panel-side auto|left|right   which side the panel docks to
  --wander                       stroll around the screen when idle
  --daemon                       detach and run silently in the background
  --install-autostart            start silently at every login
  --uninstall-autostart          undo the above
  --reset-growth                 back to Lv1
  --version, --help";

/// Parse argv into a config, handling the flags that make the process exit.
/// `Ok(None)` means "this process is done, exit 0".
pub fn parse_cli(argv: Vec<String>) -> Result<Option<Cli>, String> {
    let mut cfg = config::Config::load();
    let mut reset_growth = false;
    let mut daemonize = false;
    let mut autostart_install = false;
    let mut autostart_remove = false;
    let mut passthrough: Vec<String> = Vec::new();

    let mut it = argv.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--display" => {
                let v = it.next().unwrap_or_default();
                cfg.display = v.clone();
                passthrough.extend(["--display".into(), v]);
            }
            "--port" => {
                let v = it.next().unwrap_or_default();
                cfg.port = Some(v.clone());
                passthrough.extend(["--port".into(), v]);
            }
            "--char" => {
                let v = it.next().unwrap_or_default();
                cfg.character = v.clone();
                passthrough.extend(["--char".into(), v]);
            }
            "--panel-side" => {
                let v = it.next().unwrap_or_default();
                cfg.panel_side = v.clone();
                passthrough.extend(["--panel-side".into(), v]);
            }
            "--panel" => {
                cfg.panel = true;
                passthrough.push(a);
            }
            "--no-panel" => {
                cfg.panel = false;
                passthrough.push(a);
            }
            "--wander" => {
                cfg.wander = true;
                passthrough.push(a);
            }
            "--reset-growth" => reset_growth = true,
            "--daemon" => daemonize = true,
            "--install-autostart" => autostart_install = true,
            "--uninstall-autostart" => autostart_remove = true,
            "--version" | "-V" => {
                println!("DevPet {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                return Ok(None);
            }
            other => return Err(format!("unknown argument: {other}\n\n{USAGE}")),
        }
    }

    if !["board", "desktop", "both"].contains(&cfg.display.as_str()) {
        return Err(format!("invalid --display '{}' (board|desktop|both)", cfg.display));
    }
    if !assets_gen::CHAR_NAMES.iter().take(3).any(|n| *n == cfg.character) {
        return Err(format!("invalid --char '{}' (clawd|beemo|grogu)", cfg.character));
    }
    if !["auto", "left", "right"].contains(&cfg.panel_side.as_str()) {
        return Err(format!("invalid --panel-side '{}' (auto|left|right)", cfg.panel_side));
    }

    if autostart_remove {
        println!("{}", autostart::uninstall().map_err(|e| e.to_string())?);
        return Ok(None);
    }
    if autostart_install {
        println!("{}", autostart::install(&passthrough).map_err(|e| e.to_string())?);
        return Ok(None);
    }
    if daemonize && daemon::detach(&passthrough).map_err(|e| e.to_string())? {
        return Ok(None);
    }

    cfg.save();
    Ok(Some(Cli { cfg, reset_growth }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn rejects_bad_values() {
        assert!(parse_cli(args(&["--display", "hologram"])).is_err());
        assert!(parse_cli(args(&["--char", "yoda"])).is_err());
        assert!(parse_cli(args(&["--nope"])).is_err());
    }

    #[test]
    fn help_and_version_exit_quietly() {
        assert!(parse_cli(args(&["--help"])).unwrap().is_none());
        assert!(parse_cli(args(&["--version"])).unwrap().is_none());
    }
}
