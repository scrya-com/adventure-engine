//! winit → `InputEvent` translation.
//!
//! Single function [`from_winit`] that the engine calls inside its
//! event loop. Returns `None` for events we don't care about (e.g.
//! mouse motion with no buttons held that aren't raw motion).

use adventure_core::math::Vec2;

use crate::event::{InputEvent, MouseButton};
use crate::key::KeyCode;

/// Translate a winit event into an `InputEvent`, if applicable.
///
/// Pass the winit event + the current logical window size (for
/// normalising mouse coordinates if you want them in 0..1; pass
/// `None` to keep pixel coords).
pub fn from_winit(
    event: &winit::event::WindowEvent,
    _window_size: Option<(f32, f32)>,
) -> Option<InputEvent> {
    use winit::event::WindowEvent;
    match event {
        WindowEvent::CursorMoved { position, .. } => {
            let pos = Vec2::new(position.x as f32, position.y as f32);
            Some(InputEvent::MouseMove { pos })
        }
        WindowEvent::MouseInput { state, button, .. } => {
            let pos = Vec2::ZERO; // Caller can patch in the latest pos via state.
            let mb = map_mouse_button(*button);
            match state {
                winit::event::ElementState::Pressed => {
                    Some(InputEvent::MouseDown { button: mb, pos })
                }
                winit::event::ElementState::Released => {
                    Some(InputEvent::MouseUp { button: mb, pos })
                }
            }
        }
        WindowEvent::MouseWheel { delta, .. } => {
            // winit gives LineDelta(x, y) or PixelDelta(x, y).
            let d = match delta {
                winit::event::MouseScrollDelta::LineDelta(x, y) => Vec2::new(*x, *y),
                winit::event::MouseScrollDelta::PixelDelta(p) => {
                    Vec2::new(p.x as f32 / 16.0, p.y as f32 / 16.0)
                }
            };
            Some(InputEvent::Wheel {
                delta: d,
                pos: Vec2::ZERO,
            })
        }
        WindowEvent::KeyboardInput { event, .. } => {
            let key = map_key(&event.physical_key)?;
            match event.state {
                winit::event::ElementState::Pressed => Some(InputEvent::KeyDown { key }),
                winit::event::ElementState::Released => Some(InputEvent::KeyUp { key }),
            }
        }
        WindowEvent::Ime(ime) => match ime {
            winit::event::Ime::Commit(s) => s.chars().next().map(|c| InputEvent::Char { c }),
            _ => None,
        },
        _ => None,
    }
}

fn map_mouse_button(b: winit::event::MouseButton) -> MouseButton {
    match b {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Other(3),
        winit::event::MouseButton::Forward => MouseButton::Other(4),
        winit::event::MouseButton::Other(idx) => MouseButton::Other(idx),
    }
}

fn map_key(code: &winit::keyboard::PhysicalKey) -> Option<KeyCode> {
    use winit::keyboard::KeyCode as WKeyCode;
    let winit::keyboard::PhysicalKey::Code(c) = code else {
        return None;
    };
    Some(match c {
        WKeyCode::ArrowUp => KeyCode::Up,
        WKeyCode::ArrowDown => KeyCode::Down,
        WKeyCode::ArrowLeft => KeyCode::Left,
        WKeyCode::ArrowRight => KeyCode::Right,
        WKeyCode::Enter | WKeyCode::NumpadEnter => KeyCode::Return,
        WKeyCode::Escape => KeyCode::Escape,
        WKeyCode::Space => KeyCode::Space,
        WKeyCode::Tab => KeyCode::Tab,
        WKeyCode::Backspace => KeyCode::Backspace,
        WKeyCode::Delete => KeyCode::Delete,
        WKeyCode::Home => KeyCode::Home,
        WKeyCode::End => KeyCode::End,
        WKeyCode::PageUp => KeyCode::PageUp,
        WKeyCode::PageDown => KeyCode::PageDown,
        WKeyCode::F1 => KeyCode::F(1),
        WKeyCode::F2 => KeyCode::F(2),
        WKeyCode::F3 => KeyCode::F(3),
        WKeyCode::F4 => KeyCode::F(4),
        WKeyCode::F5 => KeyCode::F(5),
        WKeyCode::F6 => KeyCode::F(6),
        WKeyCode::F7 => KeyCode::F(7),
        WKeyCode::F8 => KeyCode::F(8),
        WKeyCode::F9 => KeyCode::F(9),
        WKeyCode::F10 => KeyCode::F(10),
        WKeyCode::F11 => KeyCode::F(11),
        WKeyCode::F12 => KeyCode::F(12),
        WKeyCode::Digit1 => KeyCode::Digit(1),
        WKeyCode::Digit2 => KeyCode::Digit(2),
        WKeyCode::Digit3 => KeyCode::Digit(3),
        WKeyCode::Digit4 => KeyCode::Digit(4),
        WKeyCode::Digit5 => KeyCode::Digit(5),
        WKeyCode::Digit6 => KeyCode::Digit(6),
        WKeyCode::Digit7 => KeyCode::Digit(7),
        WKeyCode::Digit8 => KeyCode::Digit(8),
        WKeyCode::Digit9 => KeyCode::Digit(9),
        WKeyCode::Digit0 => KeyCode::Digit(0),
        WKeyCode::KeyA => KeyCode::Letter('a'),
        WKeyCode::KeyB => KeyCode::Letter('b'),
        WKeyCode::KeyC => KeyCode::Letter('c'),
        WKeyCode::KeyD => KeyCode::Letter('d'),
        WKeyCode::KeyE => KeyCode::Letter('e'),
        WKeyCode::KeyF => KeyCode::Letter('f'),
        WKeyCode::KeyG => KeyCode::Letter('g'),
        WKeyCode::KeyH => KeyCode::Letter('h'),
        WKeyCode::KeyI => KeyCode::Letter('i'),
        WKeyCode::KeyJ => KeyCode::Letter('j'),
        WKeyCode::KeyK => KeyCode::Letter('k'),
        WKeyCode::KeyL => KeyCode::Letter('l'),
        WKeyCode::KeyM => KeyCode::Letter('m'),
        WKeyCode::KeyN => KeyCode::Letter('n'),
        WKeyCode::KeyO => KeyCode::Letter('o'),
        WKeyCode::KeyP => KeyCode::Letter('p'),
        WKeyCode::KeyQ => KeyCode::Letter('q'),
        WKeyCode::KeyR => KeyCode::Letter('r'),
        WKeyCode::KeyS => KeyCode::Letter('s'),
        WKeyCode::KeyT => KeyCode::Letter('t'),
        WKeyCode::KeyU => KeyCode::Letter('u'),
        WKeyCode::KeyV => KeyCode::Letter('v'),
        WKeyCode::KeyW => KeyCode::Letter('w'),
        WKeyCode::KeyX => KeyCode::Letter('x'),
        WKeyCode::KeyY => KeyCode::Letter('y'),
        WKeyCode::KeyZ => KeyCode::Letter('z'),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_known_key() {
        let pk = winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space);
        assert_eq!(map_key(&pk), Some(KeyCode::Space));
    }

    #[test]
    fn map_unknown_key() {
        let pk = winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Cut);
        assert_eq!(map_key(&pk), None);
    }

    #[test]
    fn map_arrow_keys() {
        let up = winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ArrowUp);
        assert_eq!(map_key(&up), Some(KeyCode::Up));
    }
}
