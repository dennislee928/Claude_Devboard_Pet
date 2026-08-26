//! petd-lite — DevPet **firmware edition** bridge.
//!
//! Here the dev board is the brain: it owns the work-state machine, the growth
//! engine, the animation and the pet's memory (NVS). This binary only
//!
//!   1. listens for Claude Code hook events on 127.0.0.1:8127,
//!   2. forwards them to the board as raw events over USB serial,
//!   3. sends the board a status line (model / act / session count / tokens),
//!   4. mirrors whatever the board reports onto the PC screen — so you can watch
//!      the pet on your monitor with nothing installed but this one file.
//!
//! Run it with `--display board` and it never opens a window at all.
//!
//! Usage: petd-lite [--display board|desktop|both] [--panel] [--daemon] …

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use petd::board::{self, BoardMsg, BoardStatus};
use petd::providers::Hub;
use petd::server::{self, Incoming};
use petd::state::Event;
use petd::{assets_gen, desktop, Shared};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

const BACKEND: &str = "firmware";

fn main() {
    let cli = match petd::parse_cli(std::env::args().skip(1).collect()) {
        Ok(Some(c)) => c,
        Ok(None) => return,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let mut cfg = cli.cfg;
    let use_desktop = cfg.display != "board";

    // Level/XP start as placeholders: the board owns them and reports the real
    // values in its first status broadcast.
    let shared = Shared::new(&cfg, BACKEND, 1, 0, Some(petd::growth::THRESHOLDS[1]));
    {
        let mut s = shared.lock().unwrap();
        s.board_enabled = true; // the board is mandatory in this edition
    }

    let (ev_tx, ev_rx) = channel::<Incoming>();
    let (board_tx, board_rx) = channel::<BoardMsg>();
    let (status_tx, status_rx) = channel::<BoardStatus>();

    server::spawn(ev_tx.clone());
    board::spawn(board_rx, cfg.port.clone(), shared.clone(), Some(status_tx));
    if cli.reset_growth {
        eprintln!("petd-lite: --reset-growth is a board-side setting; reflash or hold BOOT to clear NVS");
    }

    // board -> screen: whatever the brain says, the window shows
    let s_shared = shared.clone();
    std::thread::spawn(move || {
        while let Ok(st) = status_rx.recv() {
            {
                let mut s = s_shared.lock().unwrap();
                s.state = st.state;
                s.level = st.level;
                s.xp = st.xp;
                s.next = st.next;
            }
            if let Some(ctx) = desktop::UI_CTX.get() {
                ctx.request_repaint();
            }
        }
    });

    // hooks -> board, plus local session tracking for the status panel
    let d_shared = shared.clone();
    std::thread::spawn(move || {
        let mut hub = Hub::new(cfg.enabled_providers(), cfg.primary(), cfg.budgets.clone());
        let mut chr = cfg.char_index();
        let mut last_poll = Instant::now() - Duration::from_secs(10);
        let mut last_status: Option<(String, String, usize)> = None;
        let _ = board_tx.send(BoardMsg::SetChar(assets_gen::CHAR_NAMES[chr].to_string()));
        loop {
            let msg = ev_rx.recv_timeout(Duration::from_millis(250));
            let now = Instant::now();
            match msg {
                Ok(m) => {
                    if let Some(h) = &m.hook {
                        hub.apply_hook(h, now);
                    }
                    match m.ev {
                        Some(Event::SetChar(i)) => {
                            chr = i.min(desktop::PICKABLE - 1);
                            cfg.character = assets_gen::CHAR_NAMES[chr].to_string();
                            cfg.save();
                            d_shared.lock().unwrap().chr = chr;
                            let _ = board_tx.send(BoardMsg::SetChar(cfg.character.clone()));
                        }
                        Some(Event::SetWander(w)) => {
                            cfg.wander = w;
                            cfg.save();
                            d_shared.lock().unwrap().wander = w;
                        }
                        Some(Event::SetPanel(p)) => {
                            cfg.panel = p;
                            cfg.save();
                            d_shared.lock().unwrap().panel = p;
                        }
                        Some(Event::SetProvider(id, on)) => {
                            cfg.providers.retain(|p| *p != id);
                            if on {
                                cfg.providers.push(id);
                            }
                            cfg.save();
                            hub.set_providers(cfg.enabled_providers(), cfg.primary());
                        }
                        Some(Event::SetPrimaryProvider(id)) => {
                            cfg.primary_provider = id;
                            cfg.save();
                            hub.set_providers(cfg.enabled_providers(), cfg.primary());
                        }
                        // everything else is the board's decision, not ours
                        Some(ev) => {
                            let _ = board_tx.send(BoardMsg::Event(ev));
                        }
                        None => {}
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }

            if last_poll.elapsed() >= Duration::from_secs(1) {
                last_poll = now;
                // Codex has no hooks, so anything its rollouts gained is
                // forwarded to the board, which owns the state machine here.
                for ev in hub.poll() {
                    let _ = board_tx.send(BoardMsg::Event(ev));
                }
                let snap = hub.snapshot(now);
                let (model, action) = snap
                    .focus()
                    .map(|f| (f.model.clone(), f.action.clone()))
                    .unwrap_or_else(|| (String::new(), String::new()));
                let active = snap.active();
                let key = (model.clone(), action.clone(), active);
                if last_status.as_ref() != Some(&key) {
                    last_status = Some(key);
                    let _ = board_tx.send(BoardMsg::Status {
                        model,
                        action,
                        sessions: active,
                        tokens: snap.session_tokens().total(),
                        percent: snap
                            .providers
                            .iter()
                            .flat_map(|p| p.windows.iter())
                            .filter_map(|w| w.used_percent)
                            .fold(-1.0f32, f32::max)
                            .round() as i16,
                    });
                }
                d_shared.lock().unwrap().usage = snap;
                if let Some(ctx) = desktop::UI_CTX.get() {
                    ctx.request_repaint();
                }
            }
        }
    });

    println!("petd-lite: {BACKEND} edition — the board is the brain");
    if use_desktop {
        if let Err(e) = desktop::run(shared, ev_tx) {
            eprintln!("petd-lite: desktop UI failed: {e}");
        }
    } else {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    }
}
