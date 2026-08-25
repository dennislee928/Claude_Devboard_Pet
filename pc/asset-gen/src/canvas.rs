//! 40x40 indexed pixel canvas used to author all sprite frames.

pub const W: i32 = 40;

pub type Rgb = [u8; 3];

// Palette (RGB888; converted to RGB332 for firmware, RGBA for PNGs)
pub const ORANGE: Rgb = [232, 115, 74]; // Clawd body
pub const ORANGE_D: Rgb = [180, 70, 40]; // Clawd shading
pub const ORANGE_L: Rgb = [255, 160, 120];
pub const WHITE: Rgb = [255, 255, 255];
pub const BLACK: Rgb = [25, 25, 32];
pub const BLUSH: Rgb = [255, 130, 130];
pub const TEAL: Rgb = [96, 200, 180]; // Beemo body
pub const TEAL_D: Rgb = [52, 140, 125];
pub const SCREEN: Rgb = [214, 246, 225]; // Beemo screen
pub const SCREEN_DIM: Rgb = [60, 90, 80];
pub const FACE: Rgb = [40, 70, 78]; // Beemo face color
pub const YELLOW: Rgb = [255, 208, 74];
pub const RED: Rgb = [235, 70, 70];
pub const RED_D: Rgb = [160, 40, 40];
pub const BLUE: Rgb = [92, 140, 235];
pub const GREEN: Rgb = [96, 200, 110];
pub const GRAY: Rgb = [150, 152, 162];
pub const GRAY_D: Rgb = [92, 94, 104];
pub const BROWN: Rgb = [150, 100, 60];
pub const PINK: Rgb = [240, 120, 190];
pub const EGGSHELL: Rgb = [248, 238, 218];
pub const EGGSHELL_D: Rgb = [214, 192, 156];
pub const PURPLE: Rgb = [150, 100, 220];

#[derive(Clone, PartialEq)]
pub struct Canvas {
    pub px: Vec<Option<Rgb>>, // row-major 40x40, None = transparent
}

impl Canvas {
    pub fn new() -> Self {
        Canvas { px: vec![None; (W * W) as usize] }
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if (0..W).contains(&x) && (0..W).contains(&y) {
            self.px[(y * W + x) as usize] = Some(c);
        }
    }

    pub fn clear_px(&mut self, x: i32, y: i32) {
        if (0..W).contains(&x) && (0..W).contains(&y) {
            self.px[(y * W + x) as usize] = None;
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgb) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, c);
            }
        }
    }

    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, c: Rgb) {
        for x in x0..=x1 {
            self.set(x, y, c);
        }
    }

    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, c: Rgb) {
        for y in y0..=y1 {
            self.set(x, y, c);
        }
    }

    /// Filled ellipse.
    pub fn ellipse(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: Rgb) {
        for y in cy - ry..=cy + ry {
            for x in cx - rx..=cx + rx {
                let dx = (x - cx) as f32 / rx as f32;
                let dy = (y - cy) as f32 / ry as f32;
                if dx * dx + dy * dy <= 1.0 {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// Ellipse outline (1px, coarse).
    pub fn ring(&mut self, cx: i32, cy: i32, rx: i32, ry: i32, c: Rgb) {
        for y in cy - ry..=cy + ry {
            for x in cx - rx..=cx + rx {
                let dx = (x - cx) as f32 / rx as f32;
                let dy = (y - cy) as f32 / ry as f32;
                let d = dx * dx + dy * dy;
                let edge = 1.0 - 1.6 / rx.min(ry).max(1) as f32;
                if d <= 1.0 && d >= edge * edge {
                    self.set(x, y, c);
                }
            }
        }
    }

    /// Draw a diagonal line (integer steps).
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for i in 0..=steps {
            let x = x0 + (x1 - x0) * i / steps;
            let y = y0 + (y1 - y0) * i / steps;
            self.set(x, y, c);
        }
    }

    /// Overlay another canvas on top (its opaque pixels win).
    pub fn blit(&mut self, other: &Canvas) {
        for i in 0..self.px.len() {
            if let Some(c) = other.px[i] {
                self.px[i] = Some(c);
            }
        }
    }
}

/// A single authored animation frame: pixels + vertical bob offset that any
/// level-accessory overlay must be shifted by to stay glued to the body.
pub struct Frame {
    pub canvas: Canvas,
    pub bob: i8,
}

pub struct Anim {
    pub frames: Vec<Frame>,
    pub dur_ms: u16,
}

pub fn rgb332(c: Rgb) -> u8 {
    (c[0] & 0xE0) | ((c[1] & 0xE0) >> 3) | (c[2] >> 6)
}
