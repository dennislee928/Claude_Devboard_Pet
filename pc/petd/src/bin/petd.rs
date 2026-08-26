//! petd — DevPet **standalone edition**: the whole pet runs on this machine.
//!
//! Receives Claude Code hook events over HTTP (from pet-hook), runs the
//! work-state machine, the growth engine and the session tracker, and shows the
//! pet on the desktop, on an ESP32 board, or both. No dev board required.
//!
//! Usage: petd [--display board|desktop|both] [--char clawd|beemo|grogu]
//!             [--panel] [--daemon] [--install-autostart] …

// Release builds get no console window; debug builds keep it for development.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use petd::board::{self, BoardMsg};
use petd::growth::{self, Growth};
use petd::providers::Hub;
use petd::server::{self, Incoming};
use petd::state::{self, Event, Machine};
use petd::{assets_gen, desktop, Shared};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::time::{Duration, Instant};

const BACKEND: &str = "standalone";

/// The single usage percentage worth showing on a 240x240 screen: the tightest
/// window across every watched provider. -1 when nothing knows a limit.
fn board_percent(snap: &petd::providers::Snapshot) -> i16 {
    snap.providers
        .iter()
        .flat_map(|p| p.windows.iter())
        .filter_map(|w| w.used_percent)
        .fold(-1.0f32, f32::max)
        .round() as i16
}

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

    let mut growth = if cli.reset_growth { Growth::default() } else { Growth::load() };
    if cli.reset_growth {
        growth.save();
    }
    let display_mode = cfg.display.clone();
    let use_board = cfg.display != "desktop";
    let use_desktop = cfg.display != "board";
    let shared = Shared::new(&cfg, BACKEND, growth.level(), growth.xp, growth.next_threshold());

    let (ev_tx, ev_rx) = channel::<Incoming>();
    let (board_tx, board_rx) = channel::<BoardMsg>();

    server::spawn(ev_tx.clone());
    if use_board {
        board::spawn(board_rx, cfg.port.clone(), shared.clone(), None);
    }

    // dispatcher: state machine + growth + sessions, feeds shared/board/UI
    let d_shared = shared.clone();
    std::thread::spawn(move || {
        let mut machine = Machine::new(Instant::now());
        let mut hub = Hub::new(cfg.enabled_providers(), cfg.primary(), cfg.budgets.clone());
        let mut chr = cfg.char_index();
        let mut last_minute = Instant::now();
        let mut last_poll = Instant::now() - Duration::from_secs(10);
        let mut last_pushed: Option<(usize, usize, u8)> = None;
        let mut last_status: Option<(String, String, usize)> = None;
        let mut last_pet_xp: Option<Instant> = None;
        let mut last_feed_xp: Option<Instant> = None;
        loop {
            let msg = ev_rx.recv_timeout(Duration::from_millis(250));
            let now = Instant::now();
            let mut leveled = false;
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
                        Some(ev) => {
                            let mut gain = state::xp_for(&ev);
                            // interaction XP is rate-limited so it can't be farmed
                            match ev {
                                Event::Petted if last_pet_xp.is_none_or(|t| now - t > Duration::from_secs(60)) => {
                                    last_pet_xp = Some(now);
                                    gain += 1;
                                }
                                Event::Feed if last_feed_xp.is_none_or(|t| now - t > Duration::from_secs(600)) => {
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
                        None => {}
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }

            // Token usage comes from the providers' own transcripts; polling
            // once a second is plenty and costs only the bytes appended since
            // the last read. Codex has no hooks, so whatever its rollout files
            // gained also becomes pet events — but only when Codex is the
            // provider the user picked to drive the pet.
            let snap = if last_poll.elapsed() >= Duration::from_secs(1) {
                last_poll = now;
                for ev in hub.poll() {
                    let mut gain = state::xp_for(&ev);
                    if let Some(bonus) = machine.on_event(&ev, now) {
                        gain += bonus;
                    }
                    if gain > 0 {
                        leveled |= growth.add(gain);
                    }
                }
                growth.save();
                Some(hub.snapshot(now))
            } else {
                None
            };

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
                if let Some(snap) = &snap {
                    s.usage = snap.clone();
                }
            }

            if use_board {
                let key = (machine.state, chr, level);
                if last_pushed != Some(key) {
                    last_pushed = Some(key);
                    let _ = board_tx.send(BoardMsg::Render {
                        state: assets_gen::STATE_NAMES[machine.state].to_string(),
                        chr: assets_gen::CHAR_NAMES[chr].to_string(),
                        level,
                    });
                }
                if let Some(snap) = &snap {
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
                            percent: board_percent(snap),
                        });
                    }
                }
            }

            if let Some(ctx) = desktop::UI_CTX.get() {
                ctx.request_repaint();
            }
        }
    });

    println!("petd: {BACKEND} edition, display '{display_mode}'");
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
