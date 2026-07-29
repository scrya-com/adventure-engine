//! Tiny 5×7 bitmap font → flat-coloured quads (no external font files).
//!
//! Good enough for verb bars, hotspot labels, and status lines until
//! adventure-ui gains a real text phase.

use adventure_core::math::Vec2;
use adventure_render2d::{DrawEffect, DrawElement, ShaderKind, TextureId, Tint, UvRect};

/// Pixel scale for one glyph cell (width includes 1px gap).
pub const CELL_W: f32 = 6.0;
#[allow(dead_code)]
pub const CELL_H: f32 = 8.0;

/// 5×7 bitmaps for printable ASCII subset (space + !../~). Missing → block.
fn glyph(c: char) -> [u8; 7] {
    // Rows are 5 bits, MSB = left pixel.
    match c.to_ascii_uppercase() {
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100, 0b00000],
        '\'' => [0b00100, 0b00100, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        ',' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b01000, 0b00000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ':' => [0b00000, 0b00100, 0b00000, 0b00000, 0b00100, 0b00000, 0b00000],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b00000, 0b00100],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        _ => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111],
    }
}

fn push_px(
    out: &mut Vec<DrawElement>,
    white: TextureId,
    x: f32,
    y: f32,
    s: f32,
    tint: Tint,
    layer: i32,
) {
    out.push(DrawElement {
        layer,
        shader: ShaderKind::Sprite,
        effect: DrawEffect::NONE,
        texture: white,
        uv: UvRect::FULL,
        tint,
        positions: vec![
            Vec2::new(x, y),
            Vec2::new(x + s, y),
            Vec2::new(x + s, y + s),
            Vec2::new(x, y),
            Vec2::new(x + s, y + s),
            Vec2::new(x, y + s),
        ],
        uvs: vec![
            Vec2::ZERO,
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::ZERO,
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ],
    });
}

/// Draw `text` at top-left `(x,y)` with pixel size `scale` (1.0 = 5×7 cells).
pub fn draw_text(
    out: &mut Vec<DrawElement>,
    white: TextureId,
    x: f32,
    y: f32,
    text: &str,
    scale: f32,
    tint: Tint,
    layer: i32,
) {
    let px = scale;
    let mut cx = x;
    for ch in text.chars() {
        if ch == '\n' {
            continue;
        }
        let rows = glyph(ch);
        for (ry, row) in rows.iter().enumerate() {
            for bit in 0..5 {
                if row & (1 << (4 - bit)) != 0 {
                    push_px(
                        out,
                        white,
                        cx + bit as f32 * px,
                        y + ry as f32 * px,
                        px,
                        tint,
                        layer,
                    );
                }
            }
        }
        cx += CELL_W * scale;
    }
}

/// Approximate width of a string in pixels.
pub fn text_width(text: &str, scale: f32) -> f32 {
    text.chars().count() as f32 * CELL_W * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_emit_pixels() {
        let mut v = Vec::new();
        draw_text(
            &mut v,
            TextureId::NONE,
            0.0,
            0.0,
            "HI",
            1.0,
            Tint::IDENTITY,
            0,
        );
        assert!(!v.is_empty());
        assert!(text_width("HI", 2.0) > text_width("H", 2.0));
    }
}
