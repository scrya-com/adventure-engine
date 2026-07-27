//! Engine main loop and integration layer.
//!
//! Owns the [`bevy_ecs::World`], schedules systems per phase, drives
//! render/audio/input/ui subsystems, and runs the frame loop. See
//! `docs/ARCHITECTURE.md` for the per-frame data flow.

#![deny(missing_docs)]
