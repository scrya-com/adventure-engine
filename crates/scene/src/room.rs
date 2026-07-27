//! Room: a single screenful of game world.
//!
//! A scene contains one or more rooms. Each room has a background image,
//! hotspots, props, and named spawn points for walkers.

use std::collections::BTreeMap;

use adventure_core::{AssetId, SmolStr};
use adventure_core::math::Vec2;
use serde::{Deserialize, Serialize};

use crate::hotspot::Hotspot;
use crate::prop::Prop;
use crate::transform::Facing;
use crate::walk_graph_bridge::WalkGraphBridge;

/// A named spawn point in a room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spawn {
    /// Stable spawn id within the room (e.g. `"entry"`).
    pub id: SmolStr,
    /// Position in normalized background space.
    pub point: Vec2,
    /// Initial facing.
    pub facing: Facing,
    /// Optional depth (0 near, 1 far). Defaults to mid.
    #[serde(default = "default_depth")]
    pub depth: f32,
}

fn default_depth() -> f32 {
    0.5
}

/// A room: a single screenful of game world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Room {
    /// Stable room id (matches the key in [`crate::scene::Scene::rooms`]).
    pub id: SmolStr,
    /// Background image asset.
    pub background: AssetId,
    /// Walk graph for character locomotion.
    #[serde(default)]
    pub walk_graph: WalkGraphBridge,
    /// Hotspots (clickable regions).
    #[serde(default)]
    pub hotspots: Vec<Hotspot>,
    /// Props (placed sprites).
    #[serde(default)]
    pub props: Vec<Prop>,
    /// Named spawn points.
    #[serde(default)]
    pub spawns: BTreeMap<SmolStr, Spawn>,
    /// Ambient music (loops while player is in this room).
    #[serde(default)]
    pub ambient_music: Option<AssetId>,
    /// Ambient SFX (e.g. birds, wind).
    #[serde(default)]
    pub ambient_sfx: Option<AssetId>,
}

impl Room {
    /// Find a hotspot by id.
    pub fn hotspot(&self, id: &str) -> Option<&Hotspot> {
        self.hotspots.iter().find(|h| h.id == id)
    }

    /// Find a hotspot containing a point (topmost first).
    pub fn hotspot_at(&self, p: Vec2) -> Option<&Hotspot> {
        self.hotspots.iter().find(|h| h.contains(p))
    }

    /// Find a spawn by id.
    pub fn spawn(&self, id: &str) -> Option<&Spawn> {
        self.spawns.get(id)
    }

    /// Find a prop by id.
    pub fn prop(&self, id: &str) -> Option<&Prop> {
        self.props.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hotspot::{Cursor, HotspotKind, OnClick};

    fn make_test_room() -> Room {
        Room {
            id: SmolStr::new("clearing"),
            background: AssetId::from_path("bg/clearing"),
            walk_graph: WalkGraphBridge::default(),
            hotspots: vec![Hotspot {
                id: SmolStr::new("door"),
                kind: HotspotKind::Exit,
                polygon: vec![
                    Vec2::new(0.4, 0.6),
                    Vec2::new(0.5, 0.6),
                    Vec2::new(0.5, 0.8),
                    Vec2::new(0.4, 0.8),
                ],
                cursor: Cursor::Default,
                on_click: OnClick::Action(SmolStr::new("enter_cottage")),
            }],
            props: vec![],
            spawns: {
                let mut m = BTreeMap::new();
                m.insert(
                    SmolStr::new("entry"),
                    Spawn {
                        id: SmolStr::new("entry"),
                        point: Vec2::new(0.2, 0.9),
                        facing: Facing::East,
                        depth: 0.0,
                    },
                );
                m
            },
            ambient_music: Some(AssetId::from_path("music/forest")),
            ambient_sfx: None,
        }
    }

    #[test]
    fn find_hotspot_at_point() {
        let r = make_test_room();
        let h = r.hotspot_at(Vec2::new(0.45, 0.7));
        assert!(h.is_some());
        assert_eq!(h.unwrap().id, "door");
    }

    #[test]
    fn no_hotspot_at_empty_point() {
        let r = make_test_room();
        assert!(r.hotspot_at(Vec2::new(0.1, 0.1)).is_none());
    }

    #[test]
    fn find_spawn() {
        let r = make_test_room();
        let s = r.spawn("entry");
        assert!(s.is_some());
        assert_eq!(s.unwrap().facing, Facing::East);
    }
}
