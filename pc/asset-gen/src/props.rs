//! Shared props drawn around either character (bubbles, tools, effects).

use crate::canvas::*;

/// Thought bubble in the top-right corner; `phase` toggles the trailing dots.
pub fn thought_bubble(c: &mut Canvas, phase: usize) {
    if phase == 0 {
        c.set(27, 12, WHITE);
        c.set(29, 9, WHITE);
    } else {
        c.set(28, 11, WHITE);
        c.set(30, 8, WHITE);
    }
    c.ellipse(33, 5, 5, 4, WHITE);
    // question mark
    c.hline(32, 34, 3, FACE);
    c.set(35, 4, FACE);
    c.set(34, 5, FACE);
    c.set(33, 6, FACE);
    c.set(33, 8, FACE);
}

/// Magnifying glass, lens centered at (cx, cy), handle to lower-left.
pub fn magnifier(c: &mut Canvas, cx: i32, cy: i32) {
    c.ring(cx, cy, 4, 4, GRAY);
    c.set(cx - 1, cy - 1, WHITE); // glint
    c.line(cx - 3, cy + 3, cx - 6, cy + 6, BROWN);
    c.line(cx - 4, cy + 3, cx - 7, cy + 6, BROWN);
}

/// Bubbling test flask at (x, y) = top-left of flask body; phase moves bubbles.
pub fn flask(c: &mut Canvas, x: i32, y: i32, phase: usize) {
    c.rect(x + 2, y, 2, 3, GRAY); // neck
    for (i, w) in [(0i32, 4i32), (1, 6), (2, 6), (3, 6)] {
        c.hline(x + 3 - w / 2, x + 2 + w / 2, y + 3 + i, GREEN);
    }
    c.hline(x, x + 5, y + 7, GREEN);
    let by = if phase == 0 { y - 2 } else { y - 4 };
    c.set(x + 1, by, GREEN);
    c.set(x + 4, by - 1, GREEN);
}

/// Hammer near (x, y); phase 0 = raised, 1 = struck down with spark.
pub fn hammer(c: &mut Canvas, x: i32, y: i32, phase: usize) {
    if phase == 0 {
        c.rect(x, y, 5, 3, GRAY_D); // head
        c.vline(x + 2, y + 3, y + 8, BROWN); // handle
    } else {
        c.rect(x - 2, y + 5, 5, 3, GRAY_D);
        c.line(x + 3, y + 8, x + 7, y + 4, BROWN);
        // impact spark
        c.set(x - 4, y + 6, YELLOW);
        c.set(x - 3, y + 4, YELLOW);
        c.set(x - 4, y + 9, YELLOW);
    }
}

/// Little beetle crawling at (x, y); phase wiggles its legs.
pub fn bug(c: &mut Canvas, x: i32, y: i32, phase: usize) {
    c.ellipse(x, y, 2, 1, PURPLE);
    c.set(x - 2, y - 1, PURPLE);
    let l = if phase == 0 { 1 } else { 0 };
    c.set(x - 1, y + 2, BLACK);
    c.set(x + 1, y + 2 - l, BLACK);
    c.set(x + 2, y + 2, BLACK);
}

/// Red exclamation mark, centered near the top; phase flashes it.
pub fn bang(c: &mut Canvas, phase: usize) {
    let col = if phase == 0 { RED } else { RED_D };
    c.rect(19, 2, 2, 6, col);
    c.rect(19, 10, 2, 2, col);
}

/// Green check mark in the top-right.
pub fn check(c: &mut Canvas) {
    c.line(27, 9, 29, 11, GREEN);
    c.line(28, 9, 30, 11, GREEN);
    c.line(30, 11, 35, 4, GREEN);
    c.line(31, 11, 36, 4, GREEN);
}

/// Wall clock in the top-right; phase moves the minute hand.
pub fn clock(c: &mut Canvas, phase: usize) {
    c.ellipse(32, 7, 5, 5, WHITE);
    c.ring(32, 7, 5, 5, GRAY_D);
    c.vline(32, 4, 7, BLACK); // hour hand up
    if phase == 0 {
        c.hline(32, 35, 7, RED);
    } else {
        c.vline(32, 7, 10, RED);
    }
}

/// Ringing bell in the top-right; phase tilts it.
pub fn bell(c: &mut Canvas, phase: usize) {
    let dx = if phase == 0 { -1 } else { 1 };
    c.set(32 + dx, 2, YELLOW);
    c.rect(30 + dx, 3, 5, 3, YELLOW);
    c.rect(29 + dx, 6, 7, 2, YELLOW);
    c.set(32 + dx, 8, GRAY_D); // clapper
    // motion ticks
    if phase == 0 {
        c.vline(26, 3, 4, YELLOW);
    } else {
        c.vline(38, 3, 4, YELLOW);
    }
}

/// Confetti sprinkled around the top half; phase shuffles positions.
pub fn confetti(c: &mut Canvas, phase: usize) {
    let pts: &[(i32, i32, Rgb)] = if phase == 0 {
        &[(6, 4, RED), (12, 7, YELLOW), (20, 3, BLUE), (28, 6, PINK), (34, 3, GREEN), (9, 12, BLUE), (31, 11, YELLOW)]
    } else if phase == 1 {
        &[(8, 6, GREEN), (14, 3, PINK), (22, 7, YELLOW), (27, 2, RED), (35, 7, BLUE), (5, 10, YELLOW), (33, 13, RED)]
    } else {
        &[(5, 7, BLUE), (13, 5, RED), (19, 8, GREEN), (25, 4, YELLOW), (32, 9, PINK), (10, 2, PINK), (36, 5, YELLOW)]
    };
    for &(x, y, col) in pts {
        c.set(x, y, col);
    }
}

/// Floating Zs; phase alternates which are visible.
pub fn zzz(c: &mut Canvas, phase: usize) {
    if phase == 0 {
        z_small(c, 28, 12);
        z_big(c, 32, 4);
    } else {
        z_small(c, 30, 8);
        z_big(c, 26, 14);
    }
}

fn z_small(c: &mut Canvas, x: i32, y: i32) {
    c.hline(x, x + 2, y, BLUE);
    c.set(x + 1, y + 1, BLUE);
    c.hline(x, x + 2, y + 2, BLUE);
}

fn z_big(c: &mut Canvas, x: i32, y: i32) {
    c.hline(x, x + 4, y, BLUE);
    c.set(x + 3, y + 1, BLUE);
    c.set(x + 2, y + 2, BLUE);
    c.set(x + 1, y + 3, BLUE);
    c.hline(x, x + 4, y + 4, BLUE);
}
