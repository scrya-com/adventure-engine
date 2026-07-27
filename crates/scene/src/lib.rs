//! Scene, Room, Hotspot, Prop data model + RON format.
//!
//! Defines the authored-data shapes for adventure scenes. See
//! `docs/DATA-FORMATS.md` for the schema reference.

#![deny(missing_docs)]

pub mod scene;
pub mod room;
pub mod hotspot;
pub mod prop;
pub mod transform;
pub mod walk_graph_bridge;

pub use hotspot::{Cursor, Hotspot, HotspotKind, OnClick};
pub use prop::{Prop, Sprite};
pub use room::{Room, Spawn};
pub use scene::Scene;
pub use transform::{Facing, Rect, Transform2D};
pub use walk_graph_bridge::WalkGraphBridge;
