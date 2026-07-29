//! UI layer: immediate-mode panels/buttons + a dialog-box overlay.
//!
//! Mirrors UE's split: [`AHUD::HitBox`] is immediate-mode
//! (re-registered every `DrawHUD`), while [`UMG`] is retained. We use
//! the same split — immediate for menus, retained for HUD.
//!
//! This crate produces [`adventure_render2d::DrawElement`]s. Text
//! rendering is deferred to a later phase (caller logs speaker / line
//! via tracing for now).

#![deny(missing_docs)]

pub mod context;
pub mod dialog_box;
pub mod input;
pub mod layout;

pub use context::{palette, ButtonState, UiContext, UI_LAYER};
pub use dialog_box::{DialogBox, DialogBoxConfig, DialogBoxOutput};
pub use input::UiInput;
pub use layout::{place, Anchor, Rect};
