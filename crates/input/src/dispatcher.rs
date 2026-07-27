//! `Dispatcher` — owns `InputState` + routes events to `Interactive`s.
//!
//! Each frame:
//!   1. Caller pushes events via [`Dispatcher::enqueue`].
//!   2. Caller invokes [`Dispatcher::flush`] with the live hotspot list
//!      + the current list of `Interactive` implementors.
//!   3. Dispatcher runs hover-enter/exit, click dispatch, and updates
//!      the polled `InputState`.
//!
//! Reference: SlateCore's `FSlateApplication::ProcessMouseEvent` +
//! subsequent routing to widgets. We skip Slate's full bubbling
//! model; only topmost-hit interactives get the events.

use adventure_core::math::Vec2;

use crate::cursor::CursorId;
use crate::event::{InputEvent, MouseButton};
use crate::interactive::{HitTest, Interactive};
use crate::pick::{pick_topmost, PickHit};
use crate::state::InputState;
use adventure_scene::hotspot::Hotspot;

/// Outcome of a flush — what happened, for the caller to react to.
#[derive(Debug, Clone, Default)]
pub struct FlushOutcome {
    /// Hotspot the cursor is currently over (None if background).
    pub hovered_hotspot: Option<smol_str::SmolStr>,
    /// Cursor to display this frame.
    pub cursor: CursorId,
    /// Clicks that landed on a hotspot this flush (button + hotspot id).
    pub clicks: Vec<(MouseButton, smol_str::SmolStr)>,
}

/// The dispatcher.
pub struct Dispatcher {
    queue: Vec<InputEvent>,
    state: InputState,
    /// Id of the hotspot currently hovered (for enter/exit edge detection).
    hovered: Option<smol_str::SmolStr>,
    /// Last cursor published (so we only emit `CursorChanged` on change).
    last_cursor: CursorId,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    /// Empty dispatcher.
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            state: InputState::new(),
            hovered: None,
            last_cursor: CursorId::DEFAULT,
        }
    }

    /// Push an event onto the queue (processed on next flush).
    pub fn enqueue(&mut self, e: InputEvent) {
        self.queue.push(e);
    }

    /// Borrow the polled state.
    pub fn state(&self) -> &InputState {
        &self.state
    }

    /// Mutably borrow the polled state.
    pub fn state_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    /// Process all queued events.
    ///
    /// `hotspots` is the live room hotspot list (for hit-testing).
    /// `interactives` is the slice of UI widgets that should also
    /// receive events; the dispatcher consults them in order and the
    /// first interactive to consume a click wins.
    ///
    /// Returns the outcome (hovered hotspot, cursor, clicks).
    pub fn flush(
        &mut self,
        hotspots: &[Hotspot],
        interactives: &mut [(&mut dyn Interactive, Vec2, i32)],
    ) -> FlushOutcome {
        let mut outcome = FlushOutcome {
            cursor: self.last_cursor.clone(),
            ..Default::default()
        };

        // Walk events in order.
        for event in self.queue.drain(..) {
            // Update polled state regardless of who handles it.
            self.state.apply(&event);

            match event {
                InputEvent::MouseMove { pos } => {
                    // Hit-test hotspots first (adventure layer).
                    let hit = pick_topmost(hotspots, pos);
                    let new_id = hit.as_ref().map(|h| h.id.clone());
                    if new_id != self.hovered {
                        self.hovered = new_id.clone();
                    }
                    outcome.hovered_hotspot = new_id;
                    outcome.cursor = cursor_for_hotspot(hotspots, &hit);

                    // Notify interactives under the mouse.
                    for (interactive, ipos, _layer) in interactives.iter_mut() {
                        let _ = interactive.hit_test(*ipos);
                    }
                }
                InputEvent::MouseDown { button, pos } => {
                    let hit = pick_topmost(hotspots, pos);
                    if let Some(h) = &hit {
                        outcome.clicks.push((button, h.id.clone()));
                    }
                    // Forward to interactive widgets (first consumer wins).
                    for (interactive, _ipos, _layer) in interactives.iter_mut() {
                        if interactive.on_click(button, pos) {
                            break;
                        }
                    }
                }
                InputEvent::MouseUp { button, pos } => {
                    for (interactive, _ipos, _layer) in interactives.iter_mut() {
                        interactive.on_release(button, pos);
                    }
                }
                _ => {}
            }
        }

        // Publish cursor-changed events if the cursor actually moved.
        if outcome.cursor != self.last_cursor {
            self.last_cursor = outcome.cursor.clone();
        }

        outcome
    }
}

/// Resolve the cursor to show for a given hotspot hit (or background).
fn cursor_for_hotspot(hotspots: &[Hotspot], hit: &Option<PickHit>) -> CursorId {
    let Some(h) = hit else {
        return CursorId::DEFAULT;
    };
    let hotspot = hotspots.iter().find(|hp| hp.id == h.id);
    match hotspot.map(|h| &h.cursor) {
        Some(adventure_scene::hotspot::Cursor::Default) => CursorId::DEFAULT,
        // We treat Named cursor in scene as a foreign cursor string.
        Some(adventure_scene::hotspot::Cursor::Named(s)) => {
            CursorId(s.clone())
        }
        None => CursorId::DEFAULT,
    }
}

/// Adapter trait — let an `Interactive`'s `hit_test` feed back into
/// the dispatcher's per-frame loop.
pub trait HitTestExt {
    /// Run a hit-test and return the result.
    fn ext_hit_test(&self, pos: Vec2) -> Option<HitTest>;
}

impl<T: Interactive + ?Sized> HitTestExt for T {
    fn ext_hit_test(&self, pos: Vec2) -> Option<HitTest> {
        self.hit_test(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adventure_scene::hotspot::{Cursor, Hotspot, HotspotKind, OnClick};

    fn sq(id: &str, min: (f32, f32), max: (f32, f32), cursor: Cursor) -> Hotspot {
        Hotspot {
            id: smol_str::SmolStr::new_inline(id),
            kind: HotspotKind::Exit,
            polygon: vec![
                Vec2::new(min.0, min.1),
                Vec2::new(max.0, min.1),
                Vec2::new(max.0, max.1),
                Vec2::new(min.0, max.1),
            ],
            cursor,
            on_click: OnClick::Action(smol_str::SmolStr::new_inline("noop")),
        }
    }

    #[test]
    fn empty_flush_is_no_op() {
        let mut d = Dispatcher::new();
        let outcome = d.flush(&[], &mut []);
        assert!(outcome.hovered_hotspot.is_none());
        assert_eq!(outcome.cursor, CursorId::DEFAULT);
        assert!(outcome.clicks.is_empty());
    }

    #[test]
    fn mouse_move_updates_state() {
        let mut d = Dispatcher::new();
        d.enqueue(InputEvent::MouseMove {
            pos: Vec2::new(42.0, 17.0),
        });
        let _ = d.flush(&[], &mut []);
        assert_eq!(d.state().mouse_pos(), Vec2::new(42.0, 17.0));
    }

    #[test]
    fn hover_enters_and_leaves_hotspot() {
        let mut d = Dispatcher::new();
        let h = sq("door", (0.0, 0.0), (1.0, 1.0), Cursor::Default);
        // Hover inside.
        d.enqueue(InputEvent::MouseMove {
            pos: Vec2::new(0.5, 0.5),
        });
        let outcome = d.flush(&[h.clone()], &mut []);
        assert_eq!(outcome.hovered_hotspot.as_deref(), Some("door"));
        // Hover outside.
        d.enqueue(InputEvent::MouseMove {
            pos: Vec2::new(2.0, 2.0),
        });
        let outcome = d.flush(&[h], &mut []);
        assert!(outcome.hovered_hotspot.is_none());
    }

    #[test]
    fn click_inside_hotspot_records_click() {
        let mut d = Dispatcher::new();
        let h = sq("door", (0.0, 0.0), (1.0, 1.0), Cursor::Default);
        d.enqueue(InputEvent::MouseDown {
            button: MouseButton::Left,
            pos: Vec2::new(0.5, 0.5),
        });
        let outcome = d.flush(&[h], &mut []);
        assert_eq!(outcome.clicks.len(), 1);
        assert_eq!(outcome.clicks[0].0, MouseButton::Left);
        assert_eq!(outcome.clicks[0].1.as_str(), "door");
    }

    #[test]
    fn click_outside_hotspot_records_nothing() {
        let mut d = Dispatcher::new();
        let h = sq("door", (0.0, 0.0), (0.5, 0.5), Cursor::Default);
        d.enqueue(InputEvent::MouseDown {
            button: MouseButton::Left,
            pos: Vec2::new(0.9, 0.9),
        });
        let outcome = d.flush(&[h], &mut []);
        assert!(outcome.clicks.is_empty());
    }

    #[test]
    fn cursor_adapts_to_hotspot_named_cursor() {
        let mut d = Dispatcher::new();
        let h = sq(
            "door",
            (0.0, 0.0),
            (1.0, 1.0),
            Cursor::Named(smol_str::SmolStr::new_inline("walk")),
        );
        d.enqueue(InputEvent::MouseMove {
            pos: Vec2::new(0.5, 0.5),
        });
        let outcome = d.flush(&[h], &mut []);
        assert_eq!(outcome.cursor.0.as_str(), "walk");
    }

    #[test]
    fn interactive_consumes_click_first_match() {
        struct Consumer {
            clicked: bool,
        }
        impl Interactive for Consumer {
            fn hit_test(&self, _pos: Vec2) -> Option<HitTest> {
                None
            }
            fn on_click(&mut self, _button: MouseButton, _pos: Vec2) -> bool {
                self.clicked = true;
                true
            }
        }
        struct Passthrough;
        impl Interactive for Passthrough {
            fn hit_test(&self, _pos: Vec2) -> Option<HitTest> {
                None
            }
            fn on_click(&mut self, _button: MouseButton, _pos: Vec2) -> bool {
                false
            }
        }

        let mut c = Consumer { clicked: false };
        let mut p = Passthrough;
        let mut interactives: Vec<(&mut dyn Interactive, Vec2, i32)> =
            vec![(&mut c, Vec2::ZERO, 0), (&mut p, Vec2::ZERO, 0)];

        let mut d = Dispatcher::new();
        d.enqueue(InputEvent::MouseDown {
            button: MouseButton::Left,
            pos: Vec2::ZERO,
        });
        d.flush(&[], interactives.as_mut_slice());

        assert!(c.clicked);
    }
}
