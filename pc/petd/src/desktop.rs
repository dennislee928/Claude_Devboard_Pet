//! Desktop pet: a borderless, always-on-top, transparent window.
//!
//! The Claude Code status panel lives in a *separate* window that docks beside
//! the pet and is repositioned whenever the pet moves, so it can never cover
//! the pet (requirement: the panel must not block the pet). The right-click
//! menu is deliberately kept shorter than the pet window for the same reason —
//! transparent areas are click-through, so the pet's own pixels are the only
//! reliable place to click a menu away.

use crate::assets_gen::{ANIMS, CHAR_NAMES, OVERLAYS, STATE_NAMES};
use crate::growth::LEVEL_NAMES;
use crate::server::Incoming;
use crate::sessions::{Snapshot, Tokens};
use crate::state::Event;
use crate::Shared;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2, ViewportBuilder, ViewportCommand, ViewportId};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

pub static UI_CTX: OnceLock<egui::Context> = OnceLock::new();

/// Character index that renders the egg (level 1) — last entry of CHAR_NAMES.
pub const EGG: usize = 3;
/// Characters the user can pick between.
pub const PICKABLE: usize = 3;

const SCALE_BY_LEVEL: [usize; 5] = [3, 3, 4, 4, 5];
const PET_AREA: f32 = 200.0; // 40 * max scale
const WIN_W: f32 = 244.0;
const WIN_H: f32 = 268.0;
const PANEL_W: f32 = 330.0;
const PANEL_H: f32 = 300.0;
const PANEL_GAP: f32 = 10.0;

pub fn run(shared: Arc<Mutex<Shared>>, tx: Sender<Incoming>) -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WIN_W, WIN_H])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
            .with_taskbar(false)
            .with_title("DevPet"),
        ..Default::default()
    };
    eframe::run_native(
        "devpet",
        opts,
        Box::new(move |cc| {
            let _ = UI_CTX.set(cc.egui_ctx.clone());
            Ok(Box::new(App {
                shared,
                tx: Arc::new(Mutex::new(tx)),
                t0: Instant::now(),
                tex: HashMap::new(),
                ovl_tex: HashMap::new(),
                pet_fx: None,
                wander_dir: 1.0,
                panel_pos: Pos2::new(60.0, 60.0),
            }))
        }),
    )
}

struct App {
    shared: Arc<Mutex<Shared>>,
    tx: Arc<Mutex<Sender<Incoming>>>,
    t0: Instant,
    tex: HashMap<(usize, usize, usize), TextureHandle>,
    ovl_tex: HashMap<(usize, usize), TextureHandle>,
    pet_fx: Option<Instant>, // hearts animation after petting/feeding
    wander_dir: f32,
    panel_pos: Pos2,
}

fn decode_png(bytes: &[u8]) -> egui::ColorImage {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().expect("bad embedded png");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("bad embedded png");
    egui::ColorImage::from_rgba_unmultiplied([info.width as usize, info.height as usize], &buf[..info.buffer_size()])
}

/// 12345 -> "12.3k", 1234567 -> "1.2M"
pub fn human(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1e3),
        _ => format!("{:.1}M", n as f64 / 1e6),
    }
}

fn send(tx: &Arc<Mutex<Sender<Incoming>>>, ev: Event) {
    if let Ok(t) = tx.lock() {
        let _ = t.send(ev.into());
    }
}

impl App {
    fn frame_tex(&mut self, ctx: &egui::Context, chr: usize, state: usize, idx: usize) -> TextureHandle {
        self.tex
            .entry((chr, state, idx))
            .or_insert_with(|| {
                let img = decode_png(ANIMS[chr][state].frames[idx].0);
                ctx.load_texture(format!("f{chr}_{state}_{idx}"), img, TextureOptions::NEAREST)
            })
            .clone()
    }

    fn overlay_tex(&mut self, ctx: &egui::Context, chr: usize, lvl_idx: usize) -> TextureHandle {
        self.ovl_tex
            .entry((chr, lvl_idx))
            .or_insert_with(|| {
                let img = decode_png(OVERLAYS[chr][lvl_idx]);
                ctx.load_texture(format!("o{chr}_{lvl_idx}"), img, TextureOptions::NEAREST)
            })
            .clone()
    }

    /// Dock the status panel next to the pet: right of it when there is room,
    /// otherwise left. Never on top of it.
    fn place_panel(&mut self, ctx: &egui::Context, side_pref: &str) {
        let (outer, mon) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        let Some(outer) = outer else { return };
        let mon_w = mon.map(|m| m.x).unwrap_or(1920.0);
        let right = outer.max.x + PANEL_GAP;
        let left = outer.min.x - PANEL_W - PANEL_GAP;
        let x = match side_pref {
            "left" if left >= 0.0 => left,
            "right" if right + PANEL_W <= mon_w => right,
            _ if right + PANEL_W <= mon_w => right,
            _ if left >= 0.0 => left,
            // no room either side: sit below the pet rather than over it
            _ => outer.min.x,
        };
        let y = if x == outer.min.x { outer.max.y + PANEL_GAP } else { outer.min.y };
        self.panel_pos = Pos2::new(x.max(0.0), y.max(0.0));
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (state, chr, level, xp, next, wander, panel, sessions) = {
            let s = self.shared.lock().unwrap();
            (
                s.state,
                s.chr,
                s.level,
                s.xp,
                s.next,
                s.wander,
                s.panel,
                s.sessions.clone(),
            )
        };
        let draw_char = if level == 1 { EGG } else { chr.min(PICKABLE - 1) };
        let anim = &ANIMS[draw_char][state];
        // several agents working at once makes the pet visibly busier
        let dur = if sessions.active > 1 { (anim.dur_ms as u64 * 2 / 3).max(80) } else { anim.dur_ms as u64 };
        let elapsed = self.t0.elapsed().as_millis() as u64;
        let idx = ((elapsed / dur) % anim.frames.len() as u64) as usize;
        let bob = anim.frames[idx].1 as f32;
        let scale = SCALE_BY_LEVEL[(level - 1) as usize] as f32;

        let frame_tex = self.frame_tex(ctx, draw_char, state, idx);
        let overlay = if level >= 3 && draw_char < PICKABLE {
            Some(self.overlay_tex(ctx, draw_char, (level - 3) as usize))
        } else {
            None
        };
        let frac = match next {
            Some(n) => {
                let base = crate::growth::THRESHOLDS[(level - 1) as usize];
                ((xp - base) as f32 / (n - base) as f32).clamp(0.0, 1.0)
            }
            None => 1.0,
        };

        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(WIN_W, WIN_H), Sense::click_and_drag());
            if resp.dragged() || resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if resp.double_clicked() {
                send(&self.tx, Event::ToggleSleep);
            } else if resp.clicked() {
                send(&self.tx, Event::Petted);
                self.pet_fx = Some(Instant::now());
            }

            // pet, bottom-centred so it "stands" on the XP bar
            let size = 40.0 * scale;
            let pet_rect = Rect::from_min_size(
                Pos2::new(rect.center().x - size / 2.0, rect.top() + (PET_AREA - size)),
                Vec2::splat(size),
            );
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            let painter = ui.painter();
            painter.image(frame_tex.id(), pet_rect, uv, Color32::WHITE);
            if let Some(o) = overlay {
                painter.image(o.id(), pet_rect.translate(Vec2::new(0.0, bob * scale)), uv, Color32::WHITE);
            }

            // floating hearts after petting/feeding
            if let Some(t) = self.pet_fx {
                let e = t.elapsed().as_secs_f32();
                if e < 2.0 {
                    let fade = ((2.0 - e) / 2.0 * 255.0) as u8;
                    for (dx, spd, sz) in [(-30.0, 24.0, 14.0), (2.0, 34.0, 18.0), (32.0, 28.0, 12.0)] {
                        painter.text(
                            Pos2::new(pet_rect.center().x + dx, pet_rect.top() + 6.0 - e * spd),
                            egui::Align2::CENTER_CENTER,
                            "♥",
                            egui::FontId::proportional(sz),
                            Color32::from_rgba_unmultiplied(240, 90, 120, fade),
                        );
                    }
                } else {
                    self.pet_fx = None;
                }
            }

            // XP bar + level tag + how many agents are busy right now
            let bar = Rect::from_min_size(Pos2::new(rect.left() + 20.0, rect.top() + PET_AREA + 4.0), Vec2::new(rect.width() - 40.0, 6.0));
            painter.rect_filled(bar, 3.0, Color32::from_black_alpha(140));
            let mut fill = bar;
            fill.set_width(bar.width() * frac);
            painter.rect_filled(fill, 3.0, Color32::from_rgb(96, 200, 110));
            let tag = if sessions.active > 0 {
                format!("Lv{} {} · {} · {}⚡", level, LEVEL_NAMES[(level - 1) as usize], STATE_NAMES[state], sessions.active)
            } else {
                format!("Lv{} {} · {}", level, LEVEL_NAMES[(level - 1) as usize], STATE_NAMES[state])
            };
            painter.text(
                Pos2::new(rect.center().x, bar.bottom() + 4.0),
                egui::Align2::CENTER_TOP,
                tag,
                egui::FontId::proportional(10.0),
                Color32::from_white_alpha(190),
            );

            let menu_open = resp
                .context_menu(|ui| {
                    // Kept strictly shorter than the window so part of the pet
                    // always stays visible and clickable to dismiss it.
                    ui.set_min_width(180.0);
                    ui.spacing_mut().item_spacing.y = 3.0;
                    let mut p = panel;
                    if ui.checkbox(&mut p, "📊 Claude status panel").changed() {
                        send(&self.tx, Event::SetPanel(p));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        for (i, name) in CHAR_NAMES.iter().enumerate().take(PICKABLE) {
                            if ui.radio(chr == i, *name).clicked() {
                                send(&self.tx, Event::SetChar(i));
                                ui.close_menu();
                            }
                        }
                    });
                    if level == 1 {
                        ui.small("🥚 egg hatches into your character at Lv2");
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("🍪 Feed").clicked() {
                            send(&self.tx, Event::Feed);
                            self.pet_fx = Some(Instant::now());
                            ui.close_menu();
                        }
                        if ui.button("💤 Nap").clicked() {
                            send(&self.tx, Event::ToggleSleep);
                            ui.close_menu();
                        }
                    });
                    let mut w = wander;
                    if ui.checkbox(&mut w, "Wander when idle").changed() {
                        send(&self.tx, Event::SetWander(w));
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            ui.close_menu();
                        }
                        if ui.button("🗕").on_hover_text("Minimize").clicked() {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                            ui.close_menu();
                        }
                        if ui.button("Quit").clicked() {
                            std::process::exit(0);
                        }
                    });
                })
                .is_some();

            // idle wandering: stroll along the screen, bounce off the edges
            if wander && state == crate::state::IDLE && !menu_open && self.pet_fx.is_none() && !resp.dragged() {
                let (outer, mon) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
                if let (Some(outer), Some(mon)) = (outer, mon) {
                    let mut x = outer.min.x + 3.0 * self.wander_dir;
                    if x <= 0.0 {
                        self.wander_dir = 1.0;
                        x = 0.0;
                    } else if x + outer.width() >= mon.x {
                        self.wander_dir = -1.0;
                        x = mon.x - outer.width();
                    }
                    ctx.send_viewport_cmd(ViewportCommand::OuterPosition(Pos2::new(x, outer.min.y)));
                }
            }
        });

        if panel {
            let side = self.shared.lock().unwrap().panel_side.clone();
            self.place_panel(ctx, &side);
            let shared = self.shared.clone();
            let tx = self.tx.clone();
            let pos = self.panel_pos;
            ctx.show_viewport_deferred(
                ViewportId::from_hash_of("devpet_status"),
                ViewportBuilder::default()
                    .with_title("Claude Code status")
                    .with_inner_size([PANEL_W, PANEL_H])
                    .with_position(pos)
                    .with_decorations(false)
                    .with_always_on_top()
                    .with_taskbar(false)
                    .with_resizable(false),
                move |ctx, _class| {
                    panel_ui(ctx, &shared, &tx);
                    ctx.request_repaint_after(std::time::Duration::from_millis(400));
                },
            );
        }

        let repaint_ms = if self.pet_fx.is_some() { 40 } else { (dur / 2).max(60) };
        ctx.request_repaint_after(std::time::Duration::from_millis(repaint_ms));
    }
}

fn tokens_line(t: &Tokens) -> String {
    format!("{} in · {} out · {} cached", human(t.input), human(t.output), human(t.cache_read + t.cache_write))
}

/// The status panel: what Claude Code is doing, per session, right now.
fn panel_ui(ctx: &egui::Context, shared: &Arc<Mutex<Shared>>, tx: &Arc<Mutex<Sender<Incoming>>>) {
    let (level, xp, next, backend, board_connected, snap): (u8, u64, Option<u64>, &'static str, bool, Snapshot) = {
        let s = shared.lock().unwrap();
        (s.level, s.xp, s.next, s.backend, s.board_connected, s.sessions.clone())
    };
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.heading("DevPet");
            ui.small(format!("· {backend} edition"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").on_hover_text("Hide panel").clicked() {
                    send(tx, Event::SetPanel(false));
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Lv{level} {}", LEVEL_NAMES[(level - 1) as usize])).strong());
            match next {
                Some(n) => {
                    let base = crate::growth::THRESHOLDS[(level - 1) as usize];
                    let frac = ((xp - base) as f32 / (n - base) as f32).clamp(0.0, 1.0);
                    ui.add(egui::ProgressBar::new(frac).text(format!("XP {xp}/{n}")).desired_height(13.0));
                }
                None => {
                    ui.label(format!("XP {xp} — max level"));
                }
            }
        });
        ui.separator();

        if snap.sessions.is_empty() {
            ui.label("No Claude Code sessions seen yet.");
            ui.small("Install the hooks (hooks/settings.snippet.json) and start a session.");
        } else {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{} session(s)", snap.sessions.len())).strong());
                ui.small(format!("· {} working now", snap.active));
            });
        }

        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
            for s in &snap.sessions {
                ui.add_space(3.0);
                ui.horizontal(|ui| {
                    let (dot, col) = if s.busy { ("●", Color32::from_rgb(96, 200, 110)) } else { ("○", Color32::GRAY) };
                    ui.colored_label(col, dot);
                    ui.label(egui::RichText::new(&s.project).strong());
                    ui.small(format!("· {}", s.model));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.small(human(s.tokens.total()));
                    });
                });
                ui.small(if s.action.is_empty() { "—".to_string() } else { s.action.clone() });
                ui.small(
                    egui::RichText::new(format!(
                        "{} prompts · {} tools · {} · idle {}s",
                        s.prompts,
                        s.tool_calls,
                        tokens_line(&s.tokens),
                        s.idle_secs
                    ))
                    .weak(),
                );
                ui.separator();
            }
        });

        ui.horizontal(|ui| {
            ui.small(egui::RichText::new("Tracked sessions").strong());
            ui.small(tokens_line(&snap.session_tokens));
        });
        ui.horizontal(|ui| {
            ui.small(egui::RichText::new("All time").strong());
            ui.small(format!("{} tokens · {} prompts", human(snap.lifetime_tokens.total()), snap.lifetime_prompts));
        });

        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("🍪 Feed").clicked() {
                send(tx, Event::Feed);
            }
            if ui.button("💤 Nap").clicked() {
                send(tx, Event::ToggleSleep);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.small(if backend == "firmware" {
                    if board_connected {
                        "🔌 board: brain online"
                    } else {
                        "⚠ board: disconnected"
                    }
                } else if board_connected {
                    "🔌 board: mirroring"
                } else {
                    "PC only"
                });
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_counts_are_compact() {
        assert_eq!(human(999), "999");
        assert_eq!(human(12_345), "12.3k");
        assert_eq!(human(2_500_000), "2.5M");
    }
}
