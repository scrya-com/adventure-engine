//! Engine: main loop + bevy_ecs World + systems.
//!
//! Owns the [`World`], schedules systems per phase, drives
//! render/audio/input/ui subsystems. See `docs/ARCHITECTURE.md` for
//! the per-frame data flow.
//!
//! Phase 4 (this crate's MVP):
//!   * Components: [`Transform2D`], [`Walker`], [`Player`]
//!   * Systems: [`walker_tick_system`], [`click_to_walk_system`]
//!   * Per-frame helper: [`run_frame`]

#![deny(missing_docs)]

pub mod components;
pub mod systems;

pub use components::{CameraOffset, Player, Transform2D, Walker};
pub use systems::{
    click_to_walk_system, scene_to_pixel, pixel_to_scene, walker_tick_system, FrameContext,
    FrameSchedule, PendingClick, SceneGraph,
};
