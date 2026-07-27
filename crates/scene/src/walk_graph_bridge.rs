//! Bridge between authored scene data and `crates/locomotion::WalkGraph`.
//!
//! Scenes serialize their walk graphs in RON; the locomotion crate owns
//! the runtime representation. This bridge is the deserialized form —
//! `crates/engine` converts it into a real `WalkGraph` at load time.

use adventure_core::SmolStr;
use adventure_core::math::Vec2;
use serde::{Deserialize, Serialize};

/// What kind of node in a walk graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum WalkNodeKind {
    /// Plain floor (walkable anywhere).
    Floor,
    /// Approach pad for a hotspot (terminal node when walking to that hotspot).
    ApproachHotspot {
        /// Target hotspot id.
        target_hotspot: SmolStr,
    },
    /// Approach pad for a prop.
    ApproachProp {
        /// Target prop id.
        target_prop: SmolStr,
    },
}

/// A node in the walk graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalkNode {
    /// Stable id within the graph (e.g. `"entry"`, `"door_pad"`).
    pub id: SmolStr,
    /// Position in normalized background space.
    pub pos: Vec2,
    /// Depth (0 near, 1 far) for perspective scaling.
    #[serde(default = "default_depth")]
    pub depth: f32,
    /// Kind (controls whether this is a terminal approach pad).
    pub kind: WalkNodeKind,
}

fn default_depth() -> f32 {
    0.5
}

/// An undirected edge between two nodes (by id).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WalkEdge {
    /// First endpoint id.
    pub a: SmolStr,
    /// Second endpoint id.
    pub b: SmolStr,
}

/// The authored form of a walk graph.
///
/// This is the RON-serialized shape. The runtime [`WalkGraph`] lives in
/// `crates/locomotion`; conversion happens at scene load.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WalkGraphBridge {
    /// Nodes.
    #[serde(default)]
    pub nodes: Vec<WalkNode>,
    /// Undirected edges.
    #[serde(default)]
    pub edges: Vec<WalkEdge>,
}

impl WalkGraphBridge {
    /// Empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Find a node by id.
    pub fn node(&self, id: &str) -> Option<&WalkNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_graph_default() {
        let g = WalkGraphBridge::default();
        assert!(g.nodes.is_empty());
        assert!(g.edges.is_empty());
    }

    #[test]
    fn find_node_by_id() {
        let g = WalkGraphBridge {
            nodes: vec![WalkNode {
                id: SmolStr::new("entry"),
                pos: Vec2::new(0.2, 0.9),
                depth: 0.0,
                kind: WalkNodeKind::Floor,
            }],
            edges: vec![],
        };
        assert!(g.node("entry").is_some());
        assert!(g.node("nope").is_none());
    }
}
