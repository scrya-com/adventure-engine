//! Headless Scrya Scene adventure engine (Dart parity port).
//!
//! Source of truth for behavior until locked: `crm-flutter/lib/scene/`.
//! See `docs/PRD_RUST_SCENE_ENGINE.md`.

#![deny(missing_docs)]

pub mod ardy_motion;
pub mod compass;
pub mod meta_action;
pub mod path_retarget;
pub mod plant;
pub mod scene_point;
pub mod verb;
pub mod walk_graph;

pub use ardy_motion::{ArdyFrame, ArdyMotion, ArdyPlayMode, ardy_play_mode_from_string};
pub use compass::{
    AvalWalkResolve, WALK_RING_KEYS, compass_from_screen_heading, compass_letter_to_walk_key,
    resolve_aval_walk_from_screen_heading, walk_ring_index, walk_ring_key,
};
pub use meta_action::{MetaAction, meta_action_token};
pub use path_retarget::{
    RetargetWalk, RetargetWalkAlong, StableGaitWindow, cycle_preferred_heading,
    pick_stable_gait_window, pick_stable_gait_window_default, retarget_ardy_walk_along,
    retarget_ardy_walk_to, should_mirror_for_walk, stable_gait_frames,
};
pub use plant::{
    AVAL_CADENCE_HZ, AVAL_MAX_DURATION_SEC, AVAL_MIN_DURATION_SEC, AVAL_STEP_NORM_PER_GAIT,
    GAIT_HOLD_FRAC, PlantController, PlantSample, SOFT_SETTLE_MS, SOFT_SETTLE_TIMEOUT_MS,
    TripTiming, aval_trip_timing, gait_locked_plant_t, hard_settle_timeout_ms, is_from_idle,
    plant_progress_t, require_visual_settle, walk_ring_hops,
};
pub use scene_point::ScenePoint;
pub use verb::Verb;
pub use walk_graph::{
    WalkGraph, WalkGraphNode, WalkNodeKind, build_default_loft_walk_graph,
    build_walk_graph_from_elements, simplify_walk_path,
};
