//! `Interactive` trait + `HitTest` — what an interactive UI element is.
//!
//! Reference: Slate's `IPointerEventHandler` (we collapse hover/click
//! + capture into a single trait; adventure UIs don't need the full
//! Slate routing dance).

use adventure_core::math::Vec2;

use crate::cursor::CursorId;
use crate::event::MouseButton;

/// Stable id for an interactive thing (hotspot, UI widget, etc.).
///
/// `0` is reserved as "no interaction" (the dispatcher returns it when
/// nothing is under the cursor).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InteractionId(pub u64);

impl InteractionId {
    /// Sentinel: no interaction.
    pub const NONE: InteractionId = InteractionId(0);

    /// Construct from a u64.
    pub const fn new(v: u64) -> Self {
        Self(v)
    }

    /// Is this the NONE sentinel?
    pub fn is_none(self) -> bool {
        self == Self::NONE
    }
}

/// Result of a hit-test for a single interactive.
#[derive(Debug, Clone, PartialEq)]
pub struct HitTest {
    /// The interactive's id.
    pub id: InteractionId,
    /// Draw layer (higher = drawn on top; used to break ties).
    pub layer: i32,
    /// Cursor to show when hovering over this interactive.
    pub cursor: CursorId,
}

/// Trait for things that can react to mouse + keyboard.
///
/// UI elements implement this; the [`crate::Dispatcher`] walks them on
/// each input event and calls the appropriate method.
pub trait Interactive {
    /// Hit-test against a point in pixel space.
    ///
    /// Returns `None` if the point misses this interactive.
    fn hit_test(&self, pos: Vec2) -> Option<HitTest>;

    /// Called once when the mouse enters this interactive.
    fn on_hover_enter(&mut self) {}

    /// Called each frame the mouse stays inside this interactive.
    fn on_hover(&mut self) {}

    /// Called once when the mouse leaves this interactive.
    fn on_hover_exit(&mut self) {}

    /// Called when a mouse button is pressed while hovering.
    ///
    /// Returns `true` if the click was consumed (suppresses further
    /// dispatch to lower-layer interactives).
    fn on_click(&mut self, _button: MouseButton, _pos: Vec2) -> bool {
        false
    }

    /// Called when a mouse button is released while hovering.
    fn on_release(&mut self, _button: MouseButton, _pos: Vec2) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Button {
        bounds: adventure_scene::room::Room,
    }

    impl Interactive for Button {
        fn hit_test(&self, pos: Vec2) -> Option<HitTest> {
            // Simple bounds test.
            let _ = (pos, &self.bounds);
            None
        }
    }

    #[test]
    fn interaction_id_default_is_none() {
        assert_eq!(InteractionId::default(), InteractionId::NONE);
        assert!(InteractionId::NONE.is_none());
        assert!(!InteractionId::new(1).is_none());
    }

    #[test]
    fn trait_default_methods_are_no_ops() {
        let mut b = Button {
            bounds: adventure_scene::room::Room {
                id: smol_str::SmolStr::new("test"),
                background: adventure_core::AssetId::from_path("bg/test"),
                walk_graph: Default::default(),
                hotspots: vec![],
                props: vec![],
                spawns: Default::default(),
                ambient_music: None,
                ambient_sfx: None,
            },
        };
        // All defaults should be callable without panic.
        b.on_hover_enter();
        b.on_hover();
        b.on_hover_exit();
        assert!(!b.on_click(MouseButton::Left, Vec2::ZERO));
        b.on_release(MouseButton::Left, Vec2::ZERO);
        assert!(b.hit_test(Vec2::ZERO).is_none());
    }
}
