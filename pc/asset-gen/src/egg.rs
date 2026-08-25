//! Egg — level-1 form of every pet. Mostly wobbles; reacts to a few states.

use crate::canvas::*;
use crate::props;

fn egg_body(c: &mut Canvas, tilt: i32, cracked: bool) {
    // two stacked ellipses make an egg silhouette; `tilt` shifts the top
    c.ellipse(20, 26, 8, 7, EGGSHELL);
    c.ellipse(20 + tilt, 20, 6, 7, EGGSHELL);
    // spots
    c.set(16 + tilt, 18, EGGSHELL_D);
    c.set(24, 27, EGGSHELL_D);
    c.set(15, 25, EGGSHELL_D);
    c.set(23 + tilt, 21, EGGSHELL_D);
    // crack with peeking eyes
    if cracked {
        c.line(15, 23, 18, 22, EGGSHELL_D);
        c.line(18, 22, 20, 24, EGGSHELL_D);
        c.line(20, 24, 23, 22, EGGSHELL_D);
        c.line(23, 22, 25, 23, EGGSHELL_D);
        c.rect(17, 25, 2, 2, BLACK);
        c.rect(22, 25, 2, 2, BLACK);
    }
    // ground shadow
    c.hline(13, 27, 33, EGGSHELL_D);
}

fn wobble(dur: u16, cracked: bool) -> Anim {
    let mut l = Canvas::new();
    egg_body(&mut l, -1, cracked);
    let mut r = Canvas::new();
    egg_body(&mut r, 1, cracked);
    Anim {
        dur_ms: dur,
        frames: vec![Frame { canvas: l, bob: 0 }, Frame { canvas: r, bob: 0 }],
    }
}

pub fn anims() -> Vec<Anim> {
    let mut v = Vec::new();
    for state in 0..13usize {
        let anim = match state {
            7 => {
                // error: rapid shake + !
                let mut a = wobble(140, true);
                for (i, f) in a.frames.iter_mut().enumerate() {
                    props::bang(&mut f.canvas, i);
                }
                a
            }
            11 => {
                // celebrating: confetti
                let mut a = wobble(180, true);
                for (i, f) in a.frames.iter_mut().enumerate() {
                    props::confetti(&mut f.canvas, i);
                }
                a
            }
            12 => {
                // sleep: still egg + Zs
                let mut a = wobble(700, false);
                for (i, f) in a.frames.iter_mut().enumerate() {
                    props::zzz(&mut f.canvas, i);
                }
                a
            }
            1 | 4 | 5 => wobble(260, true), // busy states: faster wobble
            _ => wobble(450, true),
        };
        v.push(anim);
    }
    v
}
