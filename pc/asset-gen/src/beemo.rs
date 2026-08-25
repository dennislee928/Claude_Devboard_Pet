//! Beemo — a BMO-inspired (original) teal handheld-console pet. 13 states.

use crate::canvas::*;
use crate::props;

#[derive(Clone, Copy)]
enum Face {
    Normal(i32, i32), // pupil offset
    Happy,
    Dead,
    Lidded,
    Sleep,
    Code,     // screen full of code lines, tiny happy eyes
    ErrorRed, // red screen + white X eyes
}

#[derive(Clone, Copy)]
enum Arms {
    Down,
    Up,
    Wiggle(i32), // typing wiggle phase
}

struct Pose {
    bob: i32,
    face: Face,
    arms: Arms,
}

impl Default for Pose {
    fn default() -> Self {
        Pose { bob: 0, face: Face::Normal(0, 0), arms: Arms::Down }
    }
}

fn screen_face(c: &mut Canvas, b: i32, f: Face) {
    let (sx, sy) = (13, 11 + b); // screen top-left, 14x9
    match f {
        Face::ErrorRed => c.rect(sx, sy, 14, 9, RED_D),
        Face::Sleep => c.rect(sx, sy, 14, 9, SCREEN_DIM),
        _ => c.rect(sx, sy, 14, 9, SCREEN),
    }
    match f {
        Face::Normal(dx, dy) => {
            c.rect(16 + dx, 13 + b + dy, 1, 2, FACE);
            c.rect(23 + dx, 13 + b + dy, 1, 2, FACE);
            c.set(18, 17 + b, FACE);
            c.hline(19, 20, 18 + b, FACE);
            c.set(21, 17 + b, FACE);
        }
        Face::Happy => {
            for ex in [16, 23] {
                c.set(ex - 1, 14 + b, FACE);
                c.set(ex, 13 + b, FACE);
                c.set(ex + 1, 14 + b, FACE);
            }
            c.rect(18, 16 + b, 4, 2, FACE);
            c.hline(18, 21, 16 + b, SCREEN);
            c.hline(18, 21, 16 + b, FACE);
        }
        Face::Dead | Face::ErrorRed => {
            let col = if matches!(f, Face::ErrorRed) { WHITE } else { FACE };
            for ex in [16, 23] {
                c.set(ex - 1, 13 + b, col);
                c.set(ex + 1, 13 + b, col);
                c.set(ex, 14 + b, col);
                c.set(ex - 1, 15 + b, col);
                c.set(ex + 1, 15 + b, col);
            }
            c.hline(18, 21, 18 + b, col);
        }
        Face::Lidded => {
            for ex in [16, 23] {
                c.hline(ex - 1, ex + 1, 13 + b, FACE);
                c.set(ex, 14 + b, FACE);
            }
            c.hline(18, 21, 18 + b, FACE);
        }
        Face::Sleep => {
            for ex in [16, 23] {
                c.hline(ex - 1, ex + 1, 14 + b, GRAY);
            }
        }
        Face::Code => {
            c.hline(14, 20, 12 + b, GREEN);
            c.hline(16, 24, 14 + b, YELLOW);
            c.hline(14, 18, 16 + b, WHITE);
            c.hline(16, 22, 18 + b, GREEN);
            // tiny happy eyes in the corner
            c.set(24, 17 + b, FACE);
            c.set(25, 16 + b, FACE);
        }
    }
}

fn base(p: &Pose) -> Canvas {
    let mut c = Canvas::new();
    let b = p.bob;
    // legs
    for lx in [14, 24] {
        c.rect(lx, 32 + b, 2, 3, TEAL_D);
        c.hline(lx, lx + 2, 35 + b, TEAL_D);
    }
    // arms
    match p.arms {
        Arms::Down => {
            c.line(9, 19 + b, 7, 24 + b, TEAL_D);
            c.line(30, 19 + b, 32, 24 + b, TEAL_D);
        }
        Arms::Up => {
            c.line(9, 16 + b, 6, 10 + b, TEAL_D);
            c.line(30, 16 + b, 33, 10 + b, TEAL_D);
        }
        Arms::Wiggle(t) => {
            c.line(9, 19 + b, 6, 22 + b - t * 2, TEAL_D);
            c.line(30, 19 + b, 33, 20 + b + t * 2, TEAL_D);
        }
    }
    // body with rounded corners
    c.rect(10, 8 + b, 20, 24, TEAL);
    for (cx, cy) in [(10, 8 + b), (29, 8 + b), (10, 31 + b), (29, 31 + b)] {
        c.clear_px(cx, cy);
    }
    // depth shading on right/bottom edge
    c.vline(29, 9 + b, 30 + b, TEAL_D);
    c.hline(11, 29, 31 + b, TEAL_D);
    // screen + face
    screen_face(&mut c, b, p.face);
    // controls: d-pad, buttons, speaker
    c.vline(15, 23 + b, 25 + b, YELLOW);
    c.hline(14, 16, 24 + b, YELLOW);
    c.rect(24, 23 + b, 2, 2, RED);
    c.rect(21, 26 + b, 2, 1, BLUE);
    c.set(26, 27 + b, TEAL_D);
    c.set(28, 27 + b, TEAL_D);
    c.set(27, 29 + b, TEAL_D);
    c
}

fn frame(p: Pose, deco: impl Fn(&mut Canvas)) -> Frame {
    let bob = p.bob as i8;
    let mut c = base(&p);
    deco(&mut c);
    Frame { canvas: c, bob }
}

pub fn anims() -> Vec<Anim> {
    let mut v = Vec::new();

    // 0 idle
    v.push(Anim {
        dur_ms: 400,
        frames: vec![
            frame(Pose::default(), |_| {}),
            frame(Pose { bob: 1, ..Pose::default() }, |_| {}),
            frame(Pose::default(), |_| {}),
            frame(Pose { face: Face::Lidded, ..Pose::default() }, |_| {}),
        ],
    });

    // 1 coding: own screen shows code, arms wiggle
    v.push(Anim {
        dur_ms: 160,
        frames: vec![
            frame(Pose { face: Face::Code, arms: Arms::Wiggle(0), ..Pose::default() }, |_| {}),
            frame(Pose { face: Face::Code, arms: Arms::Wiggle(1), ..Pose::default() }, |_| {}),
        ],
    });

    // 2 thinking
    v.push(Anim {
        dur_ms: 350,
        frames: vec![
            frame(Pose { face: Face::Normal(0, -1), ..Pose::default() }, |c| props::thought_bubble(c, 0)),
            frame(Pose { bob: 1, face: Face::Normal(1, -1), ..Pose::default() }, |c| props::thought_bubble(c, 1)),
        ],
    });

    // 3 searching
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { face: Face::Normal(1, 0), ..Pose::default() }, |c| props::magnifier(c, 35, 14)),
            frame(Pose { face: Face::Normal(1, -1), ..Pose::default() }, |c| props::magnifier(c, 35, 13)),
        ],
    });

    // 4 testing
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { face: Face::Normal(1, 0), ..Pose::default() }, |c| props::flask(c, 33, 10, 0)),
            frame(Pose { face: Face::Normal(1, 0), ..Pose::default() }, |c| props::flask(c, 33, 10, 1)),
        ],
    });

    // 5 building
    v.push(Anim {
        dur_ms: 220,
        frames: vec![
            frame(Pose { face: Face::Normal(1, 0), ..Pose::default() }, |c| props::hammer(c, 33, 4, 0)),
            frame(Pose { bob: 1, face: Face::Normal(1, 0), ..Pose::default() }, |c| props::hammer(c, 33, 4, 1)),
        ],
    });

    // 6 debugging
    v.push(Anim {
        dur_ms: 280,
        frames: vec![
            frame(Pose { face: Face::Normal(-1, 1), ..Pose::default() }, |c| {
                props::bug(c, 5, 34, 0);
                props::magnifier(c, 6, 29);
            }),
            frame(Pose { face: Face::Normal(-1, 1), ..Pose::default() }, |c| {
                props::bug(c, 6, 34, 1);
                props::magnifier(c, 6, 29);
            }),
        ],
    });

    // 7 error: red screen
    v.push(Anim {
        dur_ms: 200,
        frames: vec![
            frame(Pose { face: Face::ErrorRed, ..Pose::default() }, |c| props::bang(c, 0)),
            frame(Pose { bob: 1, face: Face::ErrorRed, ..Pose::default() }, |c| props::bang(c, 1)),
        ],
    });

    // 8 success
    v.push(Anim {
        dur_ms: 300,
        frames: vec![
            frame(Pose { face: Face::Happy, arms: Arms::Up, ..Pose::default() }, |c| props::check(c)),
            frame(Pose { bob: 1, face: Face::Happy, arms: Arms::Up, ..Pose::default() }, |c| props::check(c)),
        ],
    });

    // 9 waiting
    v.push(Anim {
        dur_ms: 400,
        frames: vec![
            frame(Pose { face: Face::Lidded, ..Pose::default() }, |c| props::clock(c, 0)),
            frame(Pose { bob: 1, face: Face::Lidded, ..Pose::default() }, |c| props::clock(c, 1)),
        ],
    });

    // 10 notify
    v.push(Anim {
        dur_ms: 180,
        frames: vec![
            frame(Pose { face: Face::Normal(0, -1), ..Pose::default() }, |c| props::bell(c, 0)),
            frame(Pose { bob: 1, face: Face::Normal(0, -1), ..Pose::default() }, |c| props::bell(c, 1)),
        ],
    });

    // 11 celebrating
    v.push(Anim {
        dur_ms: 160,
        frames: vec![
            frame(Pose { face: Face::Happy, arms: Arms::Up, ..Pose::default() }, |c| props::confetti(c, 0)),
            frame(Pose { bob: 1, face: Face::Happy, arms: Arms::Up, ..Pose::default() }, |c| props::confetti(c, 1)),
            frame(Pose { face: Face::Happy, arms: Arms::Up, ..Pose::default() }, |c| props::confetti(c, 2)),
        ],
    });

    // 12 sleep: dim screen
    v.push(Anim {
        dur_ms: 600,
        frames: vec![
            frame(Pose { bob: 2, face: Face::Sleep, ..Pose::default() }, |c| props::zzz(c, 0)),
            frame(Pose { bob: 2, face: Face::Sleep, ..Pose::default() }, |c| props::zzz(c, 1)),
        ],
    });

    v
}
