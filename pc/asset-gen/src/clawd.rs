//! Clawd — the pixel crab. 13 states, 40x40 frames.

use crate::canvas::*;
use crate::props;

#[derive(Clone, Copy)]
enum Eyes {
    Open(i32, i32), // pupil look offset
    Happy,          // closed smiling arcs
    Blink,
    Lidded, // half closed, bored
    Dead,   // X X
    SleepClosed, // plain closed lids, no eyeball
}

#[derive(Clone, Copy)]
enum Mouth {
    Smile,
    Frown,
    Open,
    Flat,
    Grin,
}

struct Pose {
    bob: i32,
    eyes: Eyes,
    mouth: Mouth,
    claw_l: (i32, i32),
    claw_r: (i32, i32),
    blush: bool,
}

impl Default for Pose {
    fn default() -> Self {
        Pose {
            bob: 0,
            eyes: Eyes::Open(0, 0),
            mouth: Mouth::Smile,
            claw_l: (6, 21),
            claw_r: (34, 21),
            blush: false,
        }
    }
}

fn claw(c: &mut Canvas, cx: i32, cy: i32, side: i32) {
    c.ellipse(cx, cy, 3, 3, ORANGE);
    c.set(cx, cy - 2, ORANGE_L);
    // pincer notch
    c.clear_px(cx + side * 3, cy);
    c.clear_px(cx + side * 2, cy);
    c.clear_px(cx + side * 3, cy - 1);
}

fn eyes(c: &mut Canvas, b: i32, e: Eyes) {
    for ex in [15, 25] {
        // stalk
        c.vline(ex, 13 + b, 17 + b, ORANGE_D);
        match e {
            Eyes::Open(dx, dy) => {
                c.rect(ex - 2, 8 + b, 5, 5, WHITE);
                c.rect(ex - 1 + dx, 10 + b + dy - 1, 2, 2, BLACK);
            }
            Eyes::Happy => {
                // ∪ flipped: happy closed arc ∩
                c.set(ex - 2, 11 + b, BLACK);
                c.set(ex - 1, 10 + b, BLACK);
                c.set(ex, 10 + b, BLACK);
                c.set(ex + 1, 10 + b, BLACK);
                c.set(ex + 2, 11 + b, BLACK);
            }
            Eyes::Blink => {
                c.rect(ex - 2, 8 + b, 5, 5, WHITE);
                c.hline(ex - 2, ex + 2, 11 + b, BLACK);
            }
            Eyes::Lidded => {
                c.rect(ex - 2, 8 + b, 5, 5, WHITE);
                c.rect(ex - 2, 8 + b, 5, 2, ORANGE_D);
                c.rect(ex - 1, 11 + b, 2, 1, BLACK);
            }
            Eyes::SleepClosed => {
                c.hline(ex - 2, ex + 2, 11 + b, BLACK);
                c.set(ex - 2, 12 + b, BLACK);
                c.set(ex + 2, 12 + b, BLACK);
            }
            Eyes::Dead => {
                c.set(ex - 2, 8 + b, BLACK);
                c.set(ex - 1, 9 + b, BLACK);
                c.set(ex, 10 + b, BLACK);
                c.set(ex + 1, 11 + b, BLACK);
                c.set(ex + 2, 12 + b, BLACK);
                c.set(ex + 2, 8 + b, BLACK);
                c.set(ex + 1, 9 + b, BLACK);
                c.set(ex - 1, 11 + b, BLACK);
                c.set(ex - 2, 12 + b, BLACK);
            }
        }
    }
}

fn mouth(c: &mut Canvas, b: i32, m: Mouth) {
    match m {
        Mouth::Smile => {
            c.set(18, 24 + b, BLACK);
            c.hline(19, 21, 25 + b, BLACK);
            c.set(22, 24 + b, BLACK);
        }
        Mouth::Frown => {
            c.set(18, 26 + b, BLACK);
            c.hline(19, 21, 25 + b, BLACK);
            c.set(22, 26 + b, BLACK);
        }
        Mouth::Open => {
            c.rect(19, 24 + b, 3, 3, BLACK);
        }
        Mouth::Flat => {
            c.hline(18, 22, 25 + b, BLACK);
        }
        Mouth::Grin => {
            c.rect(18, 24 + b, 5, 3, BLACK);
            c.hline(18, 22, 24 + b, WHITE);
        }
    }
}

fn base(p: &Pose) -> Canvas {
    let mut c = Canvas::new();
    let b = p.bob;
    // legs
    for lx in [12, 15, 25, 28] {
        c.vline(lx, 28 + b, 32 + b, ORANGE_D);
    }
    // arms to claws
    c.line(11, 22 + b, p.claw_l.0 + 2, p.claw_l.1, ORANGE_D);
    c.line(29, 22 + b, p.claw_r.0 - 2, p.claw_r.1, ORANGE_D);
    // body
    c.ellipse(20, 23 + b, 10, 7, ORANGE);
    // shading + highlight
    for y in (27 + b)..=(29 + b) {
        for x in 11..=29 {
            let dx = (x - 20) as f32 / 10.0;
            let dy = (y - 23 - b) as f32 / 7.0;
            if dx * dx + dy * dy <= 1.0 {
                c.set(x, y, ORANGE_D);
            }
        }
    }
    c.hline(15, 19, 18 + b, ORANGE_L);
    // claws
    claw(&mut c, p.claw_l.0, p.claw_l.1, -1);
    claw(&mut c, p.claw_r.0, p.claw_r.1, 1);
    eyes(&mut c, b, p.eyes);
    mouth(&mut c, b, p.mouth);
    if p.blush {
        c.rect(12, 22 + b, 2, 1, BLUSH);
        c.rect(26, 22 + b, 2, 1, BLUSH);
    }
    c
}

fn frame(p: Pose, deco: impl Fn(&mut Canvas)) -> Frame {
    let bob = p.bob as i8;
    let mut c = base(&p);
    deco(&mut c);
    Frame { canvas: c, bob }
}

/// Build all 13 state animations, indexed by crate::STATES order.
pub fn anims() -> Vec<Anim> {
    let mut v = Vec::new();

    // 0 idle: bob + blink
    v.push(Anim {
        dur_ms: 400,
        frames: vec![
            frame(Pose::default(), |_| {}),
            frame(Pose { bob: 1, ..Pose::default() }, |_| {}),
            frame(Pose::default(), |_| {}),
            frame(Pose { eyes: Eyes::Blink, ..Pose::default() }, |_| {}),
        ],
    });

    // 1 coding: behind a laptop, claws typing
    let laptop = |c: &mut Canvas, t: i32| {
        c.rect(13, 27, 14, 8, GRAY);
        c.rect(14, 28, 12, 6, GRAY_D);
        c.rect(19, 30, 2, 2, WHITE); // logo on the lid back
        // typing claws over the lid
        claw(c, 11, 25 - t, -1);
        claw(c, 29, 24 + t, 1);
    };
    v.push(Anim {
        dur_ms: 160,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(0, 1), claw_l: (11, 25), claw_r: (29, 25), ..Pose::default() }, move |c| laptop(c, 0)),
            frame(Pose { eyes: Eyes::Open(0, 1), claw_l: (11, 25), claw_r: (29, 25), ..Pose::default() }, move |c| laptop(c, 1)),
        ],
    });

    // 2 thinking: eyes up, thought bubble
    for _ in 0..1 {
        v.push(Anim {
            dur_ms: 350,
            frames: vec![
                frame(Pose { eyes: Eyes::Open(0, -1), mouth: Mouth::Flat, ..Pose::default() }, |c| props::thought_bubble(c, 0)),
                frame(Pose { bob: 1, eyes: Eyes::Open(1, -1), mouth: Mouth::Flat, ..Pose::default() }, |c| props::thought_bubble(c, 1)),
            ],
        });
    }

    // 3 searching: magnifier in right claw
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(1, 0), claw_r: (33, 16), ..Pose::default() }, |c| props::magnifier(c, 35, 12)),
            frame(Pose { eyes: Eyes::Open(1, -1), claw_r: (33, 15), ..Pose::default() }, |c| props::magnifier(c, 35, 11)),
        ],
    });

    // 4 testing: flask bubbling
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(1, 0), claw_r: (33, 18), ..Pose::default() }, |c| props::flask(c, 31, 8, 0)),
            frame(Pose { eyes: Eyes::Open(1, 0), claw_r: (33, 18), ..Pose::default() }, |c| props::flask(c, 31, 8, 1)),
        ],
    });

    // 5 building: hammer swings
    v.push(Anim {
        dur_ms: 220,
        frames: vec![
            frame(Pose { claw_r: (34, 18), mouth: Mouth::Flat, ..Pose::default() }, |c| props::hammer(c, 32, 6, 0)),
            frame(Pose { bob: 1, claw_r: (34, 20), mouth: Mouth::Flat, ..Pose::default() }, |c| props::hammer(c, 32, 6, 1)),
        ],
    });

    // 6 debugging: inspecting a bug with magnifier
    v.push(Anim {
        dur_ms: 280,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(-1, 1), claw_l: (8, 24), mouth: Mouth::Flat, ..Pose::default() }, |c| {
                props::bug(c, 5, 34, 0);
                props::magnifier(c, 6, 29);
            }),
            frame(Pose { eyes: Eyes::Open(-1, 1), claw_l: (8, 24), mouth: Mouth::Flat, ..Pose::default() }, |c| {
                props::bug(c, 6, 34, 1);
                props::magnifier(c, 6, 29);
            }),
        ],
    });

    // 7 error: X eyes + flashing !
    v.push(Anim {
        dur_ms: 200,
        frames: vec![
            frame(Pose { eyes: Eyes::Dead, mouth: Mouth::Open, ..Pose::default() }, |c| props::bang(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Dead, mouth: Mouth::Open, ..Pose::default() }, |c| props::bang(c, 1)),
        ],
    });

    // 8 success: happy + checkmark, claws raised
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, claw_l: (5, 15), claw_r: (35, 15), ..Pose::default() }, |c| props::check(c)),
            frame(Pose { bob: 1, eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, claw_l: (5, 17), claw_r: (35, 17), ..Pose::default() }, |c| props::check(c)),
        ],
    });

    // 9 waiting: lidded eyes + clock
    v.push(Anim {
        dur_ms: 400,
        frames: vec![
            frame(Pose { eyes: Eyes::Lidded, mouth: Mouth::Flat, ..Pose::default() }, |c| props::clock(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Lidded, mouth: Mouth::Flat, ..Pose::default() }, |c| props::clock(c, 1)),
        ],
    });

    // 10 notify: wide eyes + ringing bell
    v.push(Anim {
        dur_ms: 180,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(0, -1), mouth: Mouth::Open, ..Pose::default() }, |c| props::bell(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Open(0, -1), mouth: Mouth::Open, ..Pose::default() }, |c| props::bell(c, 1)),
        ],
    });

    // 11 celebrating: confetti, claws up
    v.push(Anim {
        dur_ms: 160,
        frames: vec![
            frame(Pose { eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, claw_l: (5, 14), claw_r: (35, 14), ..Pose::default() }, |c| props::confetti(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, claw_l: (5, 16), claw_r: (35, 16), ..Pose::default() }, |c| props::confetti(c, 1)),
            frame(Pose { eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, claw_l: (5, 14), claw_r: (35, 14), ..Pose::default() }, |c| props::confetti(c, 2)),
        ],
    });

    // 12 sleep: slumped, closed eyes, Zs
    v.push(Anim {
        dur_ms: 600,
        frames: vec![
            frame(Pose { bob: 2, eyes: Eyes::SleepClosed, mouth: Mouth::Flat, ..Pose::default() }, |c| props::zzz(c, 0)),
            frame(Pose { bob: 2, eyes: Eyes::SleepClosed, mouth: Mouth::Flat, ..Pose::default() }, |c| props::zzz(c, 1)),
        ],
    });

    v
}
