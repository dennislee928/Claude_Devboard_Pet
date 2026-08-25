//! petd — desk-pet daemon.
//!
//! Receives Claude Code hook events over HTTP (from pet-hook.exe), runs the
//! work-state machine + growth engine, and shows the pet on the ESP32 board,
//! the desktop (transparent always-on-top window), or both.
//!
//! Usage: petd [--display board|desktop|both] [--port COM9] [--char clawd|beemo] [--reset-growth]

// Release builds get no console window (logs still reach files when the
// launcher redirects stdout/stderr); debug builds keep the console for dev.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets_gen;
mod board;
mod desktop;
mod growth;
mod server;
mod state;

use growth::Growth;
use serde::{Deserialize, Serialize};
use state::{Event, Machine};
use std::path::PathBuf;
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct Shared {
    pub state: usize,
    pub chr: usize, // 0 clawd, 1 beemo
    pub level: u8,
    pub xp: u64,
    pub next: Option<u64>,
    pub board_connected: bool,
    pub board_enabled: bool,
    pub wander: bool,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
struct Config {
    display: String,   // "board" | "desktop" | "both"
    port: Option<String>,
    character: String, // "clawd" | "beemo"
    wander: bool,      // pet slowly walks across the screen while idle
}

impl Default for Config {
    fn default() -> Self {
        Config { display: "both".into(), port: None, character: "clawd".into(), wander: false }
    }
}

fn config_path() -> PathBuf {
    Growth::state_path().join("config.json")
}

impl Config {
    fn load() -> Self {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    fn save(&self) {
        let _ = std::fs::create_dir_all(Growth::state_path());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(config_path(), s);
        }
    }
}

fn main() {
    let mut cfg = Config::load();
    let mut reset = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--display" => cfg.display = args.next().unwrap_or_default(),
            "--port" => cfg.port = args.next(),
            "--char" => cfg.character = args.next().unwrap_or_default(),
            "--reset-growth" => reset = true,
            "--help" | "-h" => {
                println!("petd [--display board|desktop|both] [--port COMx] [--char clawd|beemo] [--reset-growth]");
                return;
            }
            other => {
                eprintln!("unknown arg: {other}");
                return;
            }
        }
    }
    if !["board", "desktop", "both"].contains(&cfg.display.as_str()) {
        eprintln!("invalid --display '{}' (board|desktop|both)", cfg.display);
        return;
    }
    cfg.save();

    let mut growth = if reset { Growth::default() } else { Growth::load() };
    if reset {
        growth.save();
    }
    let chr_idx = if cfg.character == "beemo" { 1 } else { 0 };
    let use_board = cfg.display != "desktop";
    let use_desktop = cfg.display != "board";
    let shared = Arc::new(Mutex::new(Shared {
        state: state::IDLE,
        chr: chr_idx,
        level: growth.level(),
        xp: growth.xp,
        next: growth.next_threshold(),
        board_connected: false,
        board_enabled: use_board,
        wander: cfg.wander,
    }));

    let (ev_tx, ev_rx) = channel::<Event>();
    let (board_tx, board_rx) = channel::<board::BoardMsg>();

    server::spawn(ev_tx.clone());
    if use_board {
        board::spawn(board_rx, cfg.port.clone(), shared.clone());
    }

    // dispatcher: state machine + growth, feeds shared/board/UI
    let d_shared = shared.clone();
    let d_cfg = cfg.clone();
    std::thread::spawn(move || {
        let mut cfg = d_cfg;
        let mut machine = Machine::new(Instant::now());
        let mut chr = chr_idx;
        let mut last_minute = Instant::now();
        let mut last_pushed: Option<(usize, usize, u8)> = None;
        let mut last_pet_xp: Option<Instant> = None;
        let mut last_feed_xp: Option<Instant> = None;
        loop {
            let ev = ev_rx.recv_timeout(Duration::from_millis(250));
            let now = Instant::now();
            let mut leveled = false;
            match ev {
                Ok(Event::SetChar(i)) => {
                    chr = i.min(1);
                    cfg.character = assets_gen::CHAR_NAMES[chr].to_string();
                    cfg.save();
                }
                Ok(Event::SetWander(w)) => {
                    cfg.wander = w;
                    cfg.save();
                    d_shared.lock().unwrap().wander = w;
                }
                Ok(ev) => {
                    let mut gain = state::xp_for(&ev);
                    // interaction XP is rate-limited so it can't be farmed
                    match ev {
                        Event::Petted if last_pet_xp.map_or(true, |t| now - t > Duration::from_secs(60)) => {
                            last_pet_xp = Some(now);
                            gain += 1;
                        }
                        Event::Feed if last_feed_xp.map_or(true, |t| now - t > Duration::from_secs(600)) => {
                            last_feed_xp = Some(now);
                            gain += 5;
                        }
                        _ => {}
                    }
                    if let Some(bonus) = machine.on_event(&ev, now) {
                        gain += bonus;
                    }
                    if gain > 0 {
                        leveled = growth.add(gain);
                        growth.save();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
            machine.tick(now);
            if machine.active() && last_minute.elapsed() >= Duration::from_secs(60) {
                last_minute = now;
                leveled |= growth.add(1);
                growth.save();
            }
            if leveled {
                machine.celebrate(now);
                println!("petd: LEVEL UP -> {} ({})", growth.level(), growth::LEVEL_NAMES[(growth.level() - 1) as usize]);
            }

            let level = growth.level();
            {
                let mut s = d_shared.lock().unwrap();
                s.state = machine.state;
                s.chr = chr;
                s.level = level;
                s.xp = growth.xp;
                s.next = growth.next_threshold();
            }
            let key = (machine.state, chr, level);
            if use_board && last_pushed != Some(key) {
                last_pushed = Some(key);
                let _ = board_tx.send(board::BoardMsg {
                    state: assets_gen::STATE_NAMES[machine.state].to_string(),
                    chr: assets_gen::CHAR_NAMES[chr].to_string(),
                    level,
                });
            }
            if let Some(ctx) = desktop::UI_CTX.get() {
                ctx.request_repaint();
            }
        }
    });

    println!("petd: display mode '{}'", cfg.display);
    if use_desktop {
        if let Err(e) = desktop::run(shared, ev_tx) {
            eprintln!("petd: desktop UI failed: {e}");
        }
    } else {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }
}
