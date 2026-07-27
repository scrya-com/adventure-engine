//! Hotspot (clickable region) data model.

use adventure_core::{AssetId, SmolStr};
use adventure_core::math::Vec2;
use serde::{Deserialize, Serialize};

use crate::transform::Facing;

/// What kind of interaction a hotspot offers.
///
/// Mirrors `crates/locomotion/src/verb.rs::Verb` but with more detail
/// specific to world-level interaction (vs. inventory-level).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotspotKind {
    /// Walk here (no other interaction).
    WalkTo,
    /// Examine / look at this.
    Examine,
    /// Use the default action.
    Use,
    /// Talk to NPC.
    Talk,
    /// Pick up an item.
    Pickup,
    /// Scene transition (e.g. door to another room).
    Exit,
}

/// Action dispatched when a hotspot is clicked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum OnClick {
    /// Trigger a named action (looked up in the action table).
    Action(SmolStr),
    /// Walk here, then dispatch a named action.
    WalkThenAction {
        /// Action to dispatch after arrival.
        action: SmolStr,
        /// Optional facing override (otherwise computed from walk direction).
        facing: Option<Facing>,
    },
    /// Transition to another room.
    ChangeRoom {
        /// Target room asset id.
        room: AssetId,
        /// Spawn point name in the target room.
        spawn: SmolStr,
    },
    /// Start a dialog tree.
    StartDialog {
        /// Dialog tree asset id.
        tree: AssetId,
        /// Optional entry node (otherwise the tree's root).
        node: Option<SmolStr>,
    },
}

/// Cursor to use when hovering over this hotspot.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub enum Cursor {
    /// Use the default cursor.
    #[default]
    Default,
    /// Use a named cursor from the cursor atlas.
    Named(SmolStr),
}

/// A clickable region in a room.
///
/// Polygons are in normalized background space `[0.0, 1.0]`, y-down.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hotspot {
    /// Stable id within the room (e.g. `"hotspot_door"`).
    pub id: SmolStr,
    /// Interaction kind.
    pub kind: HotspotKind,
    /// Polygon vertices (>= 3, simple/convex recommended).
    pub polygon: Vec<Vec2>,
    /// Cursor override on hover.
    #[serde(default)]
    pub cursor: Cursor,
    /// What happens on click.
    pub on_click: OnClick,
}

impl Hotspot {
    /// Whether a point (in normalized background space) is inside the polygon.
    ///
    /// Uses ray-casting. Returns false if `polygon` has < 3 vertices.
    pub fn contains(&self, p: Vec2) -> bool {
        point_in_polygon(p, &self.polygon)
    }
}

/// Ray-cast point-in-polygon test.
pub fn point_in_polygon(p: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_in_polygon_square() {
        let square = vec![
            Vec2::new(0.4, 0.6),
            Vec2::new(0.5, 0.6),
            Vec2::new(0.5, 0.8),
            Vec2::new(0.4, 0.8),
        ];
        assert!(point_in_polygon(Vec2::new(0.45, 0.7), &square));
        assert!(!point_in_polygon(Vec2::new(0.3, 0.7), &square));
        assert!(!point_in_polygon(Vec2::new(0.6, 0.7), &square));
    }

    #[test]
    fn point_in_polygon_triangle() {
        let tri = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.5, 1.0),
        ];
        assert!(point_in_polygon(Vec2::new(0.5, 0.3), &tri));
        assert!(!point_in_polygon(Vec2::new(0.1, 0.5), &tri));
    }

    #[test]
    fn hotspot_contains_uses_polygon() {
        let h = Hotspot {
            id: SmolStr::new("test"),
            kind: HotspotKind::Examine,
            polygon: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
            cursor: Cursor::Default,
            on_click: OnClick::Action(SmolStr::new("noop")),
        };
        assert!(h.contains(Vec2::new(0.5, 0.5)));
        assert!(!h.contains(Vec2::new(1.5, 0.5)));
    }

    #[test]
    fn onclick_serde_walk_then_action() {
        let a = OnClick::WalkThenAction {
            action: SmolStr::new("open_door"),
            facing: Some(Facing::North),
        };
        let s = ron::to_string(&a).unwrap();
        let back: OnClick = ron::from_str(&s).unwrap();
        assert_eq!(back, a);
    }
}
