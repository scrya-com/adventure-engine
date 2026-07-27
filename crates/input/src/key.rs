//! `KeyCode` — abstracted keyboard scancodes.
//!
//! We intentionally do NOT re-export `winit::event::VirtualKeyCode`
//! (deprecated in 0.30 anyway). Instead we expose a small enum that
//! covers the keys an adventure game actually uses (verbs, dialog
//! choices, save/load, console). Less common keys still arrive via
//! [`crate::event::InputEvent::Char`] for text input.

/// Named keys the engine reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// Return / Enter.
    Return,
    /// Escape.
    Escape,
    /// Spacebar.
    Space,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Number keys 1-9 (verb hotkeys / dialog choices).
    Digit(u8),
    /// Letter A-Z.
    Letter(char),
    /// Function key F1-F12.
    F(u8),
}

impl KeyCode {
    /// Is this key used for verb hotkeys?
    pub fn is_verb_hotkey(self) -> bool {
        matches!(self, KeyCode::Digit(1..=9))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_hotkey() {
        assert!(KeyCode::Digit(1).is_verb_hotkey());
        assert!(KeyCode::Digit(9).is_verb_hotkey());
        assert!(!KeyCode::Digit(0).is_verb_hotkey());
        assert!(!KeyCode::Escape.is_verb_hotkey());
    }

    #[test]
    fn letter_distinct() {
        assert_ne!(KeyCode::Letter('a'), KeyCode::Letter('b'));
    }

    #[test]
    fn arrow_keys() {
        assert_ne!(KeyCode::Up, KeyCode::Down);
        assert_ne!(KeyCode::Left, KeyCode::Right);
    }
}
