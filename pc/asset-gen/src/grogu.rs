//! Grogu — the little green one. 13 states, 40x40 frames.
//!
//! Built the same way as Clawd: a `Pose` describes ears/eyes/mouth/hands and
//! `base()` renders it, then each state decorates the frame with shared props.

use crate::canvas::*;
use crate::props;

#[derive(Clone, Copy)]
enum Eyes {
    Open(i32, i32), // pupil glint offset
    Happy,
    Blink,
    Lidded,
    Dead,
    SleepClosed,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
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
    ear_droop: i32,      // 0 = perked, positive = drooping
    hand_l: (i32, i32),
    hand_r: (i32, i32),
    blush: bool,
}

impl Default for Pose {
    fn default() -> Self {
        Pose {
            bob: 0,
            eyes: Eyes::Open(0, 0),
            mouth: Mouth::Smile,
            ear_droop: 0,
            hand_l: (10, 28),
            hand_r: (30, 28),
            blush: false,
        }
    }
}

/// One big pointed ear. `side` is -1 (left) / +1 (right); `droop` bends the tip
/// downwards, which is how Grogu shows mood.
fn ear(c: &mut Canvas, b: i32, side: i32, droop: i32) {
    let bx = 20 + side * 6;
    let by = 15 + b;
    const LEN: i32 = 12;
    for t in 0..=LEN {
        let x = bx + side * t;
        let half = 5 - t * 4 / LEN;
        let yc = by + droop * t / LEN;
        c.vline(x, yc - half, yc + half, SKIN);
        if half >= 2 {
            c.vline(x, yc - half + 1, yc + half - 1, EAR_IN);
        }
        c.set(x, yc + half, SKIN_D); // bottom edge shading
    }
}

fn eyes(c: &mut Canvas, b: i32, e: Eyes) {
    for ex in [15, 25] {
        match e {
            Eyes::Open(dx, dy) => {
                c.ellipse(ex, 15 + b, 3, 4, BLACK);
                c.set(ex - 1 + dx, 13 + b + dy, WHITE);
                c.set(ex + dx, 13 + b + dy, WHITE);
                c.set(ex - 1 + dx, 14 + b + dy, WHITE);
            }
            Eyes::Happy => {
                c.set(ex - 3, 16 + b, BLACK);
                c.set(ex - 2, 14 + b, BLACK);
                c.set(ex - 1, 13 + b, BLACK);
                c.set(ex, 13 + b, BLACK);
                c.set(ex + 1, 13 + b, BLACK);
                c.set(ex + 2, 14 + b, BLACK);
                c.set(ex + 3, 16 + b, BLACK);
            }
            Eyes::Blink => {
                c.hline(ex - 3, ex + 3, 15 + b, BLACK);
                c.hline(ex - 2, ex + 2, 16 + b, BLACK);
            }
            Eyes::Lidded => {
                c.ellipse(ex, 16 + b, 3, 3, BLACK);
                c.rect(ex - 3, 12 + b, 7, 3, SKIN);
                c.hline(ex - 3, ex + 3, 14 + b, SKIN_D);
            }
            Eyes::SleepClosed => {
                c.hline(ex - 3, ex + 3, 15 + b, BLACK);
                c.set(ex - 3, 16 + b, BLACK);
                c.set(ex + 3, 16 + b, BLACK);
            }
            Eyes::Dead => {
                for d in 0..5 {
                    c.set(ex - 2 + d, 13 + b + d, BLACK);
                    c.set(ex + 2 - d, 13 + b + d, BLACK);
                }
            }
        }
    }
}

fn mouth(c: &mut Canvas, b: i32, m: Mouth) {
    match m {
        Mouth::Smile => {
            c.set(18, 21 + b, BLACK);
            c.hline(19, 21, 22 + b, BLACK);
            c.set(22, 21 + b, BLACK);
        }
        Mouth::Frown => {
            c.set(18, 23 + b, BLACK);
            c.hline(19, 21, 22 + b, BLACK);
            c.set(22, 23 + b, BLACK);
        }
        Mouth::Open => {
            c.ellipse(20, 22 + b, 2, 2, BLACK);
        }
        Mouth::Flat => {
            c.hline(18, 22, 22 + b, BLACK);
        }
        Mouth::Grin => {
            c.rect(18, 21 + b, 5, 3, BLACK);
            c.hline(18, 22, 21 + b, WHITE);
        }
    }
}

fn base(p: &Pose) -> Canvas {
    let mut c = Canvas::new();
    let b = p.bob;

    // ears sit behind the head
    ear(&mut c, b, -1, p.ear_droop);
    ear(&mut c, b, 1, p.ear_droop);

    // robe / body
    c.ellipse(20, 31 + b, 9, 7, ROBE);
    c.rect(11, 31 + b, 18, 5, ROBE);
    for y in (33 + b)..=(36 + b) {
        c.hline(12, 28, y, ROBE_D);
    }
    // collar V
    c.line(15, 24 + b, 20, 28 + b, ROBE_D);
    c.line(25, 24 + b, 20, 28 + b, ROBE_D);
    c.hline(14, 26, 24 + b, ROBE_D);

    // head
    c.ellipse(20, 16 + b, 8, 8, SKIN);
    c.ellipse(20, 20 + b, 6, 5, SKIN);
    c.hline(14, 26, 23 + b, SKIN_D); // chin shading
    c.hline(15, 20, 10 + b, SKIN_L); // forehead highlight

    // hands poking out of the sleeves
    for (hx, hy) in [p.hand_l, p.hand_r] {
        c.ellipse(hx, hy, 2, 2, SKIN);
        c.set(hx, hy - 2, SKIN_L);
    }

    eyes(&mut c, b, p.eyes);
    mouth(&mut c, b, p.mouth);
    if p.blush {
        c.rect(12, 19 + b, 2, 1, BLUSH);
        c.rect(26, 19 + b, 2, 1, BLUSH);
    }
    c
}

fn frame(p: Pose, deco: impl Fn(&mut Canvas)) -> Frame {
    let bob = p.bob as i8;
    let mut c = base(&p);
    deco(&mut c);
    Frame { canvas: c, bob }
}

/// Build all 13 state animations, indexed by crate::STATE_NAMES order.
pub fn anims() -> Vec<Anim> {
    let mut v = Vec::new();

    // 0 idle: bob + blink, ears gently drooping
    v.push(Anim {
        dur_ms: 400,
        frames: vec![
            frame(Pose::default(), |_| {}),
            frame(Pose { bob: 1, ear_droop: 1, ..Pose::default() }, |_| {}),
            frame(Pose::default(), |_| {}),
            frame(Pose { eyes: Eyes::Blink, ..Pose::default() }, |_| {}),
        ],
    });

    // 1 coding: hands on a little console
    let console = |c: &mut Canvas, t: i32| {
        c.rect(12, 30, 16, 7, GRAY);
        c.rect(13, 31, 14, 4, GRAY_D);
        c.rect(19, 32, 2, 2, GREEN);
        c.ellipse(13, 29 - t, 2, 2, SKIN);
        c.ellipse(27, 28 + t, 2, 2, SKIN);
    };
    v.push(Anim {
        dur_ms: 160,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(0, 1), ..Pose::default() }, move |c| console(c, 0)),
            frame(Pose { eyes: Eyes::Open(0, 1), ..Pose::default() }, move |c| console(c, 1)),
        ],
    });

    // 2 thinking: ears up, using the Force on a floating rock
    v.push(Anim {
        dur_ms: 350,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(0, -1), mouth: Mouth::Flat, hand_r: (31, 22), ..Pose::default() }, |c| {
                props::thought_bubble(c, 0);
                props::force(c, 0);
            }),
            frame(Pose { bob: 1, eyes: Eyes::Open(1, -1), mouth: Mouth::Flat, hand_r: (31, 21), ..Pose::default() }, |c| {
                props::thought_bubble(c, 1);
                props::force(c, 1);
            }),
        ],
    });

    // 3 searching: magnifier held up
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(1, 0), hand_r: (31, 20), ..Pose::default() }, |c| props::magnifier(c, 34, 14)),
            frame(Pose { eyes: Eyes::Open(1, -1), hand_r: (31, 19), ..Pose::default() }, |c| props::magnifier(c, 34, 13)),
        ],
    });

    // 4 testing: bubbling flask
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(1, 0), hand_r: (31, 24), ..Pose::default() }, |c| props::flask(c, 31, 12, 0)),
            frame(Pose { eyes: Eyes::Open(1, 0), hand_r: (31, 24), ..Pose::default() }, |c| props::flask(c, 31, 12, 1)),
        ],
    });

    // 5 building: hammer swings
    v.push(Anim {
        dur_ms: 220,
        frames: vec![
            frame(Pose { mouth: Mouth::Flat, hand_r: (32, 24), ..Pose::default() }, |c| props::hammer(c, 33, 12, 0)),
            frame(Pose { bob: 1, mouth: Mouth::Flat, hand_r: (32, 26), ..Pose::default() }, |c| props::hammer(c, 33, 12, 1)),
        ],
    });

    // 6 debugging: inspecting a bug on the floor
    v.push(Anim {
        dur_ms: 280,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(-1, 1), mouth: Mouth::Flat, hand_l: (8, 27), ..Pose::default() }, |c| {
                props::bug(c, 5, 36, 0);
                props::magnifier(c, 6, 31);
            }),
            frame(Pose { eyes: Eyes::Open(-1, 1), mouth: Mouth::Flat, hand_l: (8, 27), ..Pose::default() }, |c| {
                props::bug(c, 6, 36, 1);
                props::magnifier(c, 6, 31);
            }),
        ],
    });

    // 7 error: X eyes, ears flat, flashing !
    v.push(Anim {
        dur_ms: 200,
        frames: vec![
            frame(Pose { eyes: Eyes::Dead, mouth: Mouth::Open, ear_droop: 4, ..Pose::default() }, |c| props::bang(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Dead, mouth: Mouth::Open, ear_droop: 4, ..Pose::default() }, |c| props::bang(c, 1)),
        ],
    });

    // 8 success: happy, hands up, checkmark
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, hand_l: (9, 22), hand_r: (31, 22), ear_droop: -1, ..Pose::default() }, props::check),
            frame(Pose { bob: 1, eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, hand_l: (9, 24), hand_r: (31, 24), ear_droop: -1 }, props::check),
        ],
    });

    // 9 waiting: lidded eyes, drooping ears, clock
    v.push(Anim {
        dur_ms: 400,
        frames: vec![
            frame(Pose { eyes: Eyes::Lidded, mouth: Mouth::Flat, ear_droop: 3, ..Pose::default() }, |c| props::clock(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Lidded, mouth: Mouth::Flat, ear_droop: 3, ..Pose::default() }, |c| props::clock(c, 1)),
        ],
    });

    // 10 notify: ears perked, bell ringing
    v.push(Anim {
        dur_ms: 180,
        frames: vec![
            frame(Pose { eyes: Eyes::Open(0, -1), mouth: Mouth::Open, ear_droop: -2, ..Pose::default() }, |c| props::bell(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Open(0, -1), mouth: Mouth::Open, ear_droop: -2, ..Pose::default() }, |c| props::bell(c, 1)),
        ],
    });

    // 11 celebrating: confetti, hands up
    v.push(Anim {
        dur_ms: 160,
        frames: vec![
            frame(Pose { eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, hand_l: (9, 21), hand_r: (31, 21), ear_droop: -2, ..Pose::default() }, |c| props::confetti(c, 0)),
            frame(Pose { bob: 1, eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, hand_l: (9, 23), hand_r: (31, 23), ear_droop: -2 }, |c| props::confetti(c, 1)),
            frame(Pose { eyes: Eyes::Happy, mouth: Mouth::Grin, blush: true, hand_l: (9, 21), hand_r: (31, 21), ear_droop: -2, ..Pose::default() }, |c| props::confetti(c, 2)),
        ],
    });

    // 12 sleep: slumped in the pram, Zs
    v.push(Anim {
        dur_ms: 600,
        frames: vec![
            frame(Pose { bob: 2, eyes: Eyes::SleepClosed, mouth: Mouth::Flat, ear_droop: 5, ..Pose::default() }, |c| props::zzz(c, 0)),
            frame(Pose { bob: 2, eyes: Eyes::SleepClosed, mouth: Mouth::Flat, ear_droop: 5, ..Pose::default() }, |c| props::zzz(c, 1)),
        ],
    });

    v
}
