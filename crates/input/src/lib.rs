//! Input: events, polled state, cursor resources, hotspot picking.
//!
//! Mirrors UE's `FSlateApplication` input path:
//!   * `InputEvent` ↔ `FKeyEvent` / `FPointerEvent`
//!   * `InputState`  ↔ the held-state portion of `FSlateApplication`
//!   * `pick`        ↔ `FSlateApplication::FindWindowUnderCursor` →
//!                     widget hit-test (we collapse it to hotspot
//!                     polygon hit-testing since adventure UIs don't
//!                     nest widgets the way Slate does)
//!   * `Interactive` ↔ Slate's `IPointerEventHandler` interface
//!
//! See `docs/ROADMAP.md` Phase 3.

#![deny(missing_docs)]

pub mod cursor;
pub mod dispatcher;
pub mod event;
pub mod interactive;
pub mod key;
pub mod pick;
pub mod state;
#[cfg(feature = "winit")]
pub mod winit_adapter;

pub use cursor::{Cursor, CursorId};
pub use dispatcher::Dispatcher;
pub use event::{InputEvent, MouseButton, Modifiers, MouseEvent};
pub use interactive::{HitTest, Interactive, InteractionId};
pub use key::KeyCode;
pub use pick::{pick, pick_topmost};
pub use state::InputState;
