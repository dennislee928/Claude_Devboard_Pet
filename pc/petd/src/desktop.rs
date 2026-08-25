//! Desktop pet: borderless, always-on-top, transparent egui window.

use crate::assets_gen::{ANIMS, CHAR_NAMES, OVERLAYS};
use crate::growth::LEVEL_NAMES;
use crate::state::Event;
use crate::Shared;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2, ViewportCommand};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

pub static UI_CTX: OnceLock<egui::Context> = OnceLock::new();

const SCALE_BY_LEVEL: [usize; 5] = [3, 3, 4, 4, 5];
const PET_AREA: f32 = 200.0; // 40 * max scale
// Window is deliberately larger than the context menu so the menu can never
// cover the whole pet: the pet's painted pixels are the only reliable
// click-away dismiss zone (transparent areas are click-through on Windows).
const WIN_W: f32 = 244.0;
const WIN_H: f32 = 268.0;

pub fn run(shared: Arc<Mutex<Shared>>, tx: Sender<Event>) -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WIN_W, WIN_H])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_resizable(false)
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
                tx,
                t0: Instant::now(),
                tex: HashMap::new(),
                ovl_tex: HashMap::new(),
                pet_fx: None,
                wander_dir: 1.0,
            }))
        }),
    )
}

struct App {
    shared: Arc<Mutex<Shared>>,
    tx: Sender<Event>,
    t0: Instant,
    tex: HashMap<(usize, usize, usize), TextureHandle>,
    ovl_tex: HashMap<(usize, usize), TextureHandle>,
    pet_fx: Option<Instant>, // hearts animation after petting/feeding
    wander_dir: f32,
}

fn decode_png(bytes: &[u8]) -> egui::ColorImage {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().expect("bad embedded png");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("bad embedded png");
    egui::ColorImage::from_rgba_unmultiplied(
        [info.width as usize, info.height as usize],
        &buf[..info.buffer_size()],
    )
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
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let (state, chr, level, xp, next, board_connected, board_enabled, wander) = {
            let s = self.shared.lock().unwrap();
            (s.state, s.chr, s.level, s.xp, s.next, s.board_connected, s.board_enabled, s.wander)
        };
        let draw_char = if level == 1 { 2 } else { chr };
        let anim = &ANIMS[draw_char][state];
        let elapsed = self.t0.elapsed().as_millis() as u64;
        let idx = ((elapsed / anim.dur_ms as u64) % anim.frames.len() as u64) as usize;
        let bob = anim.frames[idx].1 as f32;
        let scale = SCALE_BY_LEVEL[(level - 1) as usize] as f32;

        let frame_tex = self.frame_tex(ctx, draw_char, state, idx);
        let overlay = if level >= 3 && draw_char < 2 {
            Some(self.overlay_tex(ctx, draw_char, (level - 3) as usize))
        } else {
            None
        };

        egui::CentralPanel::default().frame(egui::Frame::none()).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(WIN_W, WIN_H), Sense::click_and_drag());
            // dragged() (not just drag_started) makes grabbing reliable even
            // when the press lands a frame before movement is detected
            if resp.dragged() || resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::StartDrag);
            }
            if resp.double_clicked() {
                let _ = self.tx.send(Event::ToggleSleep);
            } else if resp.clicked() {
                let _ = self.tx.send(Event::Petted);
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

            // XP bar + level tag
            let bar = Rect::from_min_size(
                Pos2::new(rect.left() + 20.0, rect.top() + PET_AREA + 4.0),
                Vec2::new(rect.width() - 40.0, 6.0),
            );
            painter.rect_filled(bar, 3.0, Color32::from_black_alpha(140));
            let frac = match next {
                Some(n) => {
                    let base = crate::growth::THRESHOLDS[(level - 1) as usize];
                    ((xp - base) as f32 / (n - base) as f32).clamp(0.0, 1.0)
                }
                None => 1.0,
            };
            let mut fill = bar;
            fill.set_width(bar.width() * frac);
            painter.rect_filled(fill, 3.0, Color32::from_rgb(96, 200, 110));
            painter.text(
                Pos2::new(rect.center().x, bar.bottom() + 4.0),
                egui::Align2::CENTER_TOP,
                format!("Lv{} {}", level, LEVEL_NAMES[(level - 1) as usize]),
                egui::FontId::proportional(10.0),
                Color32::from_white_alpha(180),
            );

            let menu_open = resp.context_menu(|ui| {
                // Keep this menu strictly shorter than the window (WIN_H), so
                // part of the pet always stays visible and clickable to
                // dismiss it — a full-cover menu cannot be closed at all.
                ui.set_min_width(190.0);
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(
                    egui::RichText::new(format!(
                        "Lv{} {}  ·  {}",
                        level,
                        LEVEL_NAMES[(level - 1) as usize],
                        crate::assets_gen::STATE_NAMES[state]
                    ))
                    .strong(),
                );
                match next {
                    Some(n) => {
                        ui.add(egui::ProgressBar::new(frac).text(format!("XP {xp} / {n}")).desired_height(13.0));
                    }
                    None => {
                        ui.label(format!("XP {xp} — max level!"));
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    for (i, name) in CHAR_NAMES.iter().enumerate().take(2) {
                        if ui.radio(chr == i, *name).clicked() {
                            let _ = self.tx.send(Event::SetChar(i));
                            ui.close_menu();
                        }
                    }
                });
                if level == 1 {
                    ui.small("🥚 egg hatches into your character at Lv2");
                }

                egui::CollapsingHeader::new("How XP works").show(ui, |ui| {
                    ui.small("+5 prompt · +1 tool call");
                    ui.small("+1 active minute · +3 error recovery");
                    ui.small("Lv2 Baby 100 · Lv3 Junior 400");
                    ui.small("Lv4 Senior 1200 · Lv5 Legend 3000");
                    ui.small("bigger pet + accessories per level");
                });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("🍪 Feed").clicked() {
                        let _ = self.tx.send(Event::Feed);
                        self.pet_fx = Some(Instant::now());
                        ui.close_menu();
                    }
                    if ui.button("💤 Nap").clicked() {
                        let _ = self.tx.send(Event::ToggleSleep);
                        ui.close_menu();
                    }
                });
                let mut w = wander;
                if ui.checkbox(&mut w, "Wander when idle").changed() {
                    let _ = self.tx.send(Event::SetWander(w));
                }

                ui.separator();
                if board_enabled {
                    ui.small(if board_connected { "🔌 Board: connected" } else { "⚠ Board: not connected" });
                } else {
                    ui.small("Board display off (--display desktop)");
                }
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        ui.close_menu();
                    }
                    if ui.button("🗕 Minimize").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        ui.close_menu();
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                        std::process::exit(0);
                    }
                });
            }).is_some();

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

        let repaint_ms = if self.pet_fx.is_some() { 40 } else { (anim.dur_ms as u64 / 2).max(60) };
        ctx.request_repaint_after(std::time::Duration::from_millis(repaint_ms));
    }
}
