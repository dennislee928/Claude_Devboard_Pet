//! Level accessories, drawn as transparent overlay frames aligned to the
//! character's bob=0 pose. Rendered on top of the current animation frame,
//! shifted by that frame's bob offset.
//!
//! Levels: 1 = egg (no overlay), 2 = plain, 3/4/5 = cumulative accessories.

use crate::canvas::*;

fn clawd_tie(c: &mut Canvas) {
    c.rect(19, 27, 3, 1, RED);
    c.set(20, 28, RED);
    c.rect(19, 29, 3, 2, RED_D);
}

fn clawd_glasses(c: &mut Canvas) {
    for ex in [15, 25] {
        c.hline(ex - 3, ex + 3, 8, GRAY_D);
        c.vline(ex - 3, 8, 12, GRAY_D);
        c.vline(ex + 3, 8, 12, GRAY_D);
        c.hline(ex - 3, ex + 3, 12, GRAY_D);
    }
    c.hline(18, 22, 10, GRAY_D); // bridge
}

fn crown(c: &mut Canvas, y: i32) {
    c.rect(17, y + 2, 7, 2, YELLOW);
    c.set(17, y, YELLOW);
    c.set(20, y, YELLOW);
    c.set(23, y, YELLOW);
    c.set(17, y + 1, YELLOW);
    c.set(20, y + 1, YELLOW);
    c.set(23, y + 1, YELLOW);
    c.set(20, y + 2, RED); // jewel
}

fn beemo_cartridge(c: &mut Canvas) {
    c.rect(31, 20, 5, 6, PURPLE);
    c.rect(32, 21, 3, 2, GRAY);
    c.rect(32, 24, 3, 1, GRAY_D);
}

fn beemo_headphones(c: &mut Canvas) {
    // band over the top of the body
    c.hline(13, 26, 5, GRAY_D);
    c.set(12, 6, GRAY_D);
    c.set(27, 6, GRAY_D);
    c.line(11, 7, 9, 10, GRAY_D);
    c.line(28, 7, 30, 10, GRAY_D);
    // pads
    c.rect(8, 10, 2, 5, BLUE);
    c.rect(30, 10, 2, 5, BLUE);
}

/// Returns overlays[char][level] where char: 0=clawd 1=beemo, level index 0..=2
/// maps to levels 3, 4, 5. Level 2 has no accessory; egg never has one.
pub fn overlays() -> [[Canvas; 3]; 2] {
    let mut clawd3 = Canvas::new();
    clawd_tie(&mut clawd3);
    let mut clawd4 = clawd3.clone();
    clawd_glasses(&mut clawd4);
    let mut clawd5 = clawd4.clone();
    crown(&mut clawd5, 2);

    let mut beemo3 = Canvas::new();
    beemo_cartridge(&mut beemo3);
    let mut beemo4 = beemo3.clone();
    beemo_headphones(&mut beemo4);
    let mut beemo5 = beemo4.clone();
    crown(&mut beemo5, 2);

    [[clawd3, clawd4, clawd5], [beemo3, beemo4, beemo5]]
}
