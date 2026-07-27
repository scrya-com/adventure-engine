//! `InputState` — polled snapshot of the input devices.
//!
//! Use this for "is this key currently down?" queries inside systems
//! (e.g. "is shift held during this click?"). The dispatcher owns one
//! of these and keeps it in sync.

use std::collections::HashSet;

use adventure_core::math::Vec2;

use crate::event::MouseButton;
use crate::key::KeyCode;

/// Snapshot of input devices — polled, not event-driven.
#[derive(Debug, Default)]
pub struct InputState {
    mouse_pos: Vec2,
    mouse_buttons: HashSet<MouseButton>,
    keys: HashSet<KeyCode>,
    wheel_delta_accum: Vec2,
}

impl InputState {
    /// Empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply an event to the snapshot.
    pub fn apply(&mut self, event: &crate::event::InputEvent) {
        use crate::event::InputEvent;
        match event {
            InputEvent::MouseMove { pos } => self.mouse_pos = *pos,
            InputEvent::MouseDown { button, .. } => {
                self.mouse_buttons.insert(*button);
            }
            InputEvent::MouseUp { button, .. } => {
                self.mouse_buttons.remove(button);
            }
            InputEvent::Wheel { delta, .. } => {
                self.wheel_delta_accum.x += delta.x;
                self.wheel_delta_accum.y += delta.y;
            }
            InputEvent::KeyDown { key } => {
                self.keys.insert(*key);
            }
            InputEvent::KeyUp { key } => {
                self.keys.remove(key);
            }
            _ => {}
        }
    }

    /// Current mouse position (pixel space).
    pub fn mouse_pos(&self) -> Vec2 {
        self.mouse_pos
    }

    /// Is the given mouse button currently down?
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.mouse_buttons.contains(&button)
    }

    /// Is the given key currently down?
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.keys.contains(&key)
    }

    /// Accumulated wheel delta since the last [`Self::drain_wheel`].
    pub fn wheel_delta(&self) -> Vec2 {
        self.wheel_delta_accum
    }

    /// Reset the wheel accumulator (call after each frame consumes it).
    pub fn drain_wheel(&mut self) -> Vec2 {
        let d = self.wheel_delta_accum;
        self.wheel_delta_accum = Vec2::ZERO;
        d
    }

    /// Forget all currently-held keys and buttons (e.g. on window focus loss).
    pub fn clear_pressed(&mut self) {
        self.mouse_buttons.clear();
        self.keys.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::InputEvent;

    #[test]
    fn default_is_empty() {
        let s = InputState::new();
        assert_eq!(s.mouse_pos(), Vec2::ZERO);
        assert!(!s.is_key_down(KeyCode::Space));
        assert!(!s.is_mouse_down(MouseButton::Left));
    }

    #[test]
    fn tracks_mouse_position() {
        let mut s = InputState::new();
        s.apply(&InputEvent::MouseMove {
            pos: Vec2::new(10.0, 20.0),
        });
        assert_eq!(s.mouse_pos(), Vec2::new(10.0, 20.0));
    }

    #[test]
    fn tracks_button_down_then_up() {
        let mut s = InputState::new();
        s.apply(&InputEvent::MouseDown {
            button: MouseButton::Left,
            pos: Vec2::ZERO,
        });
        assert!(s.is_mouse_down(MouseButton::Left));
        s.apply(&InputEvent::MouseUp {
            button: MouseButton::Left,
            pos: Vec2::ZERO,
        });
        assert!(!s.is_mouse_down(MouseButton::Left));
    }

    #[test]
    fn tracks_key_down_then_up() {
        let mut s = InputState::new();
        s.apply(&InputEvent::KeyDown {
            key: KeyCode::Space,
        });
        assert!(s.is_key_down(KeyCode::Space));
        s.apply(&InputEvent::KeyUp {
            key: KeyCode::Space,
        });
        assert!(!s.is_key_down(KeyCode::Space));
    }

    #[test]
    fn accumulates_wheel() {
        let mut s = InputState::new();
        s.apply(&InputEvent::Wheel {
            delta: Vec2::new(0.0, 1.0),
            pos: Vec2::ZERO,
        });
        s.apply(&InputEvent::Wheel {
            delta: Vec2::new(0.0, 2.0),
            pos: Vec2::ZERO,
        });
        assert_eq!(s.wheel_delta(), Vec2::new(0.0, 3.0));
        assert_eq!(s.drain_wheel(), Vec2::new(0.0, 3.0));
        assert_eq!(s.wheel_delta(), Vec2::ZERO);
    }

    #[test]
    fn clear_pressed_drops_state() {
        let mut s = InputState::new();
        s.apply(&InputEvent::KeyDown {
            key: KeyCode::Space,
        });
        s.apply(&InputEvent::MouseDown {
            button: MouseButton::Left,
            pos: Vec2::ZERO,
        });
        s.clear_pressed();
        assert!(!s.is_key_down(KeyCode::Space));
        assert!(!s.is_mouse_down(MouseButton::Left));
    }
}
