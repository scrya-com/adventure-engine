//! [`UiContext`] — immediate-mode builder that emits [`DrawElement`]s.
//!
//! The caller owns the element list. Each frame:
//!   1. Clear the list.
//!   2. Build a [`UiContext`] borrowing the list + this frame's [`UiInput`].
//!   3. Call `panel`, `button`, etc. — they push into the list.
//!   4. Submit the list to the renderer.
//!
//! Interaction (click/hover) is queried via the same calls — `button`
//! returns `true` if it was clicked this frame.

use adventure_core::math::Vec2;
use adventure_render2d::{
    DrawEffect, DrawElement, ShaderKind, Tint, TextureId, UvRect,
};

use crate::input::UiInput;
use crate::layout::Rect;

/// Render layer for the UI. Set high so the UI draws over the world.
pub const UI_LAYER: i32 = 100;

/// A small palette of UI colours.
pub mod palette {
    use adventure_render2d::Tint;

    /// Semi-transparent black background (e.g. dialog box panel).
    pub const PANEL_BG: Tint = Tint(glam::Vec4::new(0.05, 0.05, 0.08, 0.85));
    /// Subtle border.
    pub const PANEL_BORDER: Tint = Tint(glam::Vec4::new(0.7, 0.7, 0.75, 1.0));
    /// Default button background.
    pub const BUTTON_BG: Tint = Tint(glam::Vec4::new(0.15, 0.15, 0.2, 1.0));
    /// Hovered button background.
    pub const BUTTON_HOVER: Tint = Tint(glam::Vec4::new(0.25, 0.25, 0.35, 1.0));
    /// Active (pressed) button background.
    pub const BUTTON_ACTIVE: Tint = Tint(glam::Vec4::new(0.4, 0.4, 0.5, 1.0));
}

/// Immediate-mode Ui context. Borrows the element list for the frame.
pub struct UiContext<'a> {
    input: &'a UiInput,
    elements: &'a mut Vec<DrawElement>,
}

impl<'a> UiContext<'a> {
    /// Build a new context for this frame.
    pub fn new(elements: &'a mut Vec<DrawElement>, input: &'a UiInput) -> Self {
        Self { input, elements }
    }

    /// Access the underlying input snapshot.
    pub fn input(&self) -> &UiInput {
        self.input
    }

    /// Push a flat-coloured rectangle.
    pub fn rect(&mut self, r: Rect, tint: Tint, layer: i32) {
        self.elements.push(quad_element(r, tint, layer));
    }

    /// Push a bordered rectangle (panel). The border is `border_px` thick.
    pub fn panel(&mut self, r: Rect, bg: Tint, border: Tint, border_px: f32, layer: i32) {
        self.rect(r, bg, layer);
        // Top / bottom / left / right strips.
        let top = Rect::new(r.min, Vec2::new(r.w(), border_px));
        let bot = Rect::new(Vec2::new(r.left(), r.bottom() - border_px), Vec2::new(r.w(), border_px));
        let lft = Rect::new(r.min, Vec2::new(border_px, r.h()));
        let rgt = Rect::new(Vec2::new(r.right() - border_px, r.top()), Vec2::new(border_px, r.h()));
        for s in [top, bot, lft, rgt] {
            self.rect(s, border, layer + 1);
        }
    }

    /// Push a clickable button. Returns `true` if clicked this frame.
    ///
    /// Hovered state is captured automatically from the input snapshot.
    pub fn button(&mut self, r: Rect, layer: i32) -> ButtonState {
        let hovering = self.input.hovering(r);
        let clicked = self.input.clicked_in(r);
        let bg = if clicked {
            palette::BUTTON_ACTIVE
        } else if hovering {
            palette::BUTTON_HOVER
        } else {
            palette::BUTTON_BG
        };
        self.rect(r, bg, layer);
        // 1-px border.
        let border = if hovering {
            palette::PANEL_BORDER
        } else {
            Tint(glam::Vec4::new(0.3, 0.3, 0.35, 1.0))
        };
        let top = Rect::new(r.min, Vec2::new(r.w(), 1.0));
        let bot = Rect::new(Vec2::new(r.left(), r.bottom() - 1.0), Vec2::new(r.w(), 1.0));
        let lft = Rect::new(r.min, Vec2::new(1.0, r.h()));
        let rgt = Rect::new(Vec2::new(r.right() - 1.0, r.top()), Vec2::new(1.0, r.h()));
        for s in [top, bot, lft, rgt] {
            self.rect(s, border, layer + 1);
        }
        ButtonState { hovering, clicked }
    }
}

/// Result of a [`UiContext::button`] call.
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    /// Was the mouse over the button this frame?
    pub hovering: bool,
    /// Was the button clicked this frame?
    pub clicked: bool,
}

/// Build a flat-coloured quad element covering `r`.
pub fn quad_element(r: Rect, tint: Tint, layer: i32) -> DrawElement {
    // Two triangles, CCW: (tl, bl, br) + (tl, br, tr).
    let tl = r.min;
    let tr = Vec2::new(r.right(), r.top());
    let bl = Vec2::new(r.left(), r.bottom());
    let br = Vec2::new(r.right(), r.bottom());
    DrawElement {
        layer,
        shader: ShaderKind::Sprite,
        effect: DrawEffect::NONE,
        texture: TextureId::NONE,
        uv: UvRect::FULL,
        tint,
        positions: vec![tl, bl, br, tl, br, tr],
        uvs: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 0.0),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_pushes_one_element() {
        let mut elems = Vec::new();
        let input = UiInput::default();
        let mut ui = UiContext::new(&mut elems, &input);
        ui.rect(Rect::new(Vec2::ZERO, Vec2::new(10.0, 10.0)), Tint::IDENTITY, 0);
        assert_eq!(elems.len(), 1);
    }

    #[test]
    fn panel_pushes_five_elements() {
        let mut elems = Vec::new();
        let input = UiInput::default();
        let mut ui = UiContext::new(&mut elems, &input);
        ui.panel(
            Rect::new(Vec2::ZERO, Vec2::new(100.0, 50.0)),
            palette::PANEL_BG,
            palette::PANEL_BORDER,
            2.0,
            0,
        );
        // 1 fill + 4 border strips.
        assert_eq!(elems.len(), 5);
    }

    #[test]
    fn button_reports_click() {
        let mut elems = Vec::new();
        let input = UiInput::new(Vec2::new(50.0, 25.0), Some(Vec2::new(50.0, 25.0)));
        let mut ui = UiContext::new(&mut elems, &input);
        let r = Rect::new(Vec2::ZERO, Vec2::new(100.0, 50.0));
        let s = ui.button(r, 0);
        assert!(s.clicked);
        assert!(s.hovering);
        // 1 bg + 4 border strips.
        assert_eq!(elems.len(), 5);
    }

    #[test]
    fn button_reports_no_click_outside() {
        let mut elems = Vec::new();
        let input = UiInput::new(Vec2::new(50.0, 25.0), Some(Vec2::new(500.0, 500.0)));
        let mut ui = UiContext::new(&mut elems, &input);
        let r = Rect::new(Vec2::ZERO, Vec2::new(100.0, 50.0));
        let s = ui.button(r, 0);
        assert!(!s.clicked);
        assert!(s.hovering);
    }
}
