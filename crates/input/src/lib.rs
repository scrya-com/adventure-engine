//! Input dispatch: winit events → [`Interactive`] trait callbacks.
//!
//! Collapses UE's 6-step click dispatch chain
//! (`PlayerController::InputKey` → `GetHitResultAtScreenPosition` →
//! `PrimitiveComponent::DispatchOnClicked`) into a single
//! `pick(scene, mouse_pos)` call against hotspot polygons.

#![deny(missing_docs)]
