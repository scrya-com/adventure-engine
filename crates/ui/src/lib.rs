//! UI layer: immediate-mode menus/dialog + retained HUD/inventory.
//!
//! Mirrors UE's split: [`AHUD::HitBox`] is immediate-mode
//! (re-registered every `DrawHUD`), while [`UMG`] is retained. We use
//! the same split — immediate for menus, retained for HUD.

#![deny(missing_docs)]
