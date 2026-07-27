//! `InputEvent` — abstract events the engine consumes.
//!
//! These are independent of any windowing system. winit events are
//! translated into `InputEvent` by [`crate::winit_adapter`] (behind
//! the `winit` feature).
//!
//! Reference: UE's `FKeyEvent` / `FPointerEvent` (we collapse the two
//! into a single enum since 2D adventure UIs don't need to distinguish).

use adventure_core::math::Vec2;

use crate::cursor::CursorId;
use crate::key::KeyCode;

/// Mouse or keyboard event.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// Mouse moved to `pos` (pixel space).
    MouseMove {
        /// New mouse position in pixel coordinates.
        pos: Vec2,
    },
    /// Mouse button pressed.
    MouseDown {
        /// Which button.
        button: MouseButton,
        /// Position at press.
        pos: Vec2,
    },
    /// Mouse button released.
    MouseUp {
        /// Which button.
        button: MouseButton,
        /// Position at release.
        pos: Vec2,
    },
    /// Mouse wheel scrolled. `delta` is in lines (positive = up/right).
    Wheel {
        /// Scroll delta (lines).
        delta: Vec2,
        /// Position at scroll.
        pos: Vec2,
    },
    /// Key pressed.
    KeyDown {
        /// Which key.
        key: KeyCode,
    },
    /// Key released.
    KeyUp {
        /// Which key.
        key: KeyCode,
    },
    /// Character typed (text input).
    Char {
        /// The character.
        c: char,
    },
    /// Cursor changed (driven by hover state in the dispatcher).
    CursorChanged {
        /// New cursor id.
        cursor: CursorId,
    },
}

/// Mouse buttons (mirrors winit's MouseButton, minus the platform baggage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left button (primary).
    Left,
    /// Right button (secondary).
    Right,
    /// Middle button (wheel click).
    Middle,
    /// Other button by index (0-based).
    Other(u16),
}

/// Modifier key state at the time of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift held.
    pub shift: bool,
    /// Ctrl held.
    pub ctrl: bool,
    /// Alt held.
    pub alt: bool,
    /// Logo (Windows/Command) held.
    pub logo: bool,
}

impl Modifiers {
    /// No modifiers held.
    pub const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
        logo: false,
    };

    /// Are no modifiers held?
    pub fn is_empty(self) -> bool {
        self == Self::NONE
    }
}

/// Cursor + key snapshot of a single event.
///
/// Adventures don't usually need this much, but it lets a script
/// distinguish "click" from "shift-click".
#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    /// Position at the event.
    pub pos: Vec2,
    /// Button involved (None for MouseMove).
    pub button: Option<MouseButton>,
    /// Modifier state.
    pub modifiers: Modifiers,
}

impl MouseEvent {
    /// Build a move event at `pos` with no buttons.
    pub fn move_at(pos: Vec2) -> Self {
        Self {
            pos,
            button: None,
            modifiers: Modifiers::NONE,
        }
    }

    /// Build a button event at `pos`.
    pub fn button_at(pos: Vec2, button: MouseButton) -> Self {
        Self {
            pos,
            button: Some(button),
            modifiers: Modifiers::NONE,
        }
    }
}

/// Tiny helper to build an `InputEvent::Char` from a `&str`.
pub fn char_event(s: &str) -> Option<InputEvent> {
    s.chars().next().map(|c| InputEvent::Char { c })
}

/// All non-char primitives a key event can carry.
///
/// Kept as a re-export of [`KeyCode`] for callers who want to know what
/// `key` is in a [`InputEvent::KeyDown`] without hunting through modules.
pub use crate::key::KeyCode as Key;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_default_is_none() {
        assert_eq!(Modifiers::default(), Modifiers::NONE);
        assert!(Modifiers::NONE.is_empty());
    }

    #[test]
    fn char_event_takes_first() {
        assert_eq!(
            char_event("a"),
            Some(InputEvent::Char { c: 'a' })
        );
        assert_eq!(char_event(""), None);
    }

    #[test]
    fn mouse_event_move_at() {
        let m = MouseEvent::move_at(Vec2::new(10.0, 20.0));
        assert_eq!(m.pos, Vec2::new(10.0, 20.0));
        assert!(m.button.is_none());
    }

    #[test]
    fn input_event_clone_eq() {
        let e1 = InputEvent::KeyDown {
            key: KeyCode::Escape,
        };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }

    #[test]
    fn mouse_button_distinct() {
        assert_ne!(MouseButton::Left, MouseButton::Right);
        assert_ne!(MouseButton::Other(3), MouseButton::Other(4));
    }

    #[test]
    fn wheel_carries_position() {
        let e = InputEvent::Wheel {
            delta: Vec2::new(0.0, 1.0),
            pos: Vec2::new(100.0, 200.0),
        };
        match e {
            InputEvent::Wheel { delta, pos } => {
                assert_eq!(delta, Vec2::new(0.0, 1.0));
                assert_eq!(pos, Vec2::new(100.0, 200.0));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn cursor_changed_carries_id() {
        let e = InputEvent::CursorChanged {
            cursor: CursorId(smol_str::SmolStr::new_inline("magnifier")),
        };
        if let InputEvent::CursorChanged { cursor } = e {
            assert_eq!(cursor.0.as_str(), "magnifier");
        } else {
            panic!();
        }
    }
}
