//! Per-frame input snapshot consumed by the immediate-mode Ui layer.

use adventure_core::math::Vec2;

/// Input state the Ui needs for one frame.
///
/// Build this from the engine's input layer (e.g. drain click events
/// + read mouse pos before calling [`crate::UiContext`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct UiInput {
    /// Current mouse position in pixel space.
    pub mouse_pos: Vec2,
    /// A click that occurred during this frame, if any.
    pub click: Option<Vec2>,
}

impl UiInput {
    /// Construct from mouse pos + optional click.
    pub fn new(mouse_pos: Vec2, click: Option<Vec2>) -> Self {
        Self { mouse_pos, click }
    }

    /// True if the supplied rect was clicked this frame.
    pub fn clicked_in(&self, rect: crate::layout::Rect) -> bool {
        self.click.map_or(false, |p| rect.contains(p))
    }

    /// True if the mouse is currently hovering the supplied rect.
    pub fn hovering(&self, rect: crate::layout::Rect) -> bool {
        rect.contains(self.mouse_pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Rect;

    fn r() -> Rect {
        Rect::new(Vec2::new(10.0, 10.0), Vec2::new(100.0, 50.0))
    }

    #[test]
    fn clicked_in_inside() {
        let i = UiInput::new(Vec2::new(20.0, 20.0), Some(Vec2::new(20.0, 20.0)));
        assert!(i.clicked_in(r()));
    }

    #[test]
    fn clicked_in_outside() {
        let i = UiInput::new(Vec2::new(20.0, 20.0), Some(Vec2::new(500.0, 500.0)));
        assert!(!i.clicked_in(r()));
    }

    #[test]
    fn clicked_in_no_click() {
        let i = UiInput::new(Vec2::new(20.0, 20.0), None);
        assert!(!i.clicked_in(r()));
    }

    #[test]
    fn hovering_inside() {
        let i = UiInput::new(Vec2::new(20.0, 20.0), None);
        assert!(i.hovering(r()));
    }
}
