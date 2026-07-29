//! [`DialogTree`] — collection of nodes keyed by id.

use std::collections::BTreeMap;

use adventure_core::SmolStr;
use serde::{Deserialize, Serialize};

use crate::node::{DialogNode, NodeId};

/// A dialog tree: an entry node + all nodes reachable from it.
///
/// Loaded from `assets/dialog/<name>.dlg.ron`. The entry node id is
/// fixed; everything else is reachable via `next` / `Choice::next`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogTree {
    /// Stable tree identifier (e.g. `"npc_bob_intro"`).
    pub id: SmolStr,
    /// Id of the node to enter when this tree starts.
    pub entry: NodeId,
    /// All nodes, keyed by id.
    pub nodes: BTreeMap<NodeId, DialogNode>,
}

impl DialogTree {
    /// Parse a RON string into a [`DialogTree`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::Ron`] on parse failure.
    pub fn from_ron(s: &str) -> Result<Self, crate::DialogueError> {
        ron::from_str(s).map_err(|e| crate::DialogueError::Ron(e.to_string()))
    }

    /// Serialize to a RON string (pretty).
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::Serialize`] on serialize failure.
    pub fn to_ron(&self) -> Result<String, crate::DialogueError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| crate::DialogueError::Serialize(e.to_string()))
    }

    /// Look up a node by id.
    pub fn get(&self, id: &str) -> Option<&DialogNode> {
        self.nodes.get(id)
    }

    /// Get the entry node.
    pub fn entry_node(&self) -> Option<&DialogNode> {
        self.get(&self.entry)
    }

    /// Validate the tree: entry exists, every `next` / `Choice::next`
    /// points at a real node. Returns the first dangling reference, if any.
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::MissingEntry`] if `entry` doesn't
    /// resolve, or [`crate::DialogueError::DanglingRef`] for a `next` that
    /// points at a non-existent node.
    pub fn validate(&self) -> Result<(), crate::DialogueError> {
        if !self.nodes.contains_key(&self.entry) {
            return Err(crate::DialogueError::MissingEntry(self.entry.to_string()));
        }
        for n in self.nodes.values() {
            if let Some(next) = &n.next {
                if !self.nodes.contains_key(next) {
                    return Err(crate::DialogueError::DanglingRef {
                        from: n.id.to_string(),
                        to: next.to_string(),
                    });
                }
            }
            for c in &n.choices {
                if let Some(next) = &c.next {
                    if !self.nodes.contains_key(next) {
                        return Err(crate::DialogueError::DanglingRef {
                            from: n.id.to_string(),
                            to: next.to_string(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Choice;

    fn bob_tree() -> DialogTree {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "intro".into(),
            DialogNode::linear("intro", "Bob", "Hello.", Some("ask")),
        );
        nodes.insert(
            "ask".into(),
            DialogNode::branching(
                "ask",
                "Bob",
                "What do you want?",
                vec![
                    Choice::new("Money", Some("give")),
                    Choice::new("Nothing", None::<&str>),
                ],
            ),
        );
        nodes.insert(
            "give".into(),
            DialogNode::linear("give", "Bob", "Here you go.", None::<&str>)
                .with_on_enter("add_tag(\"State.NPC.Bob.Gave\")"),
        );
        DialogTree {
            id: "bob".into(),
            entry: "intro".into(),
            nodes,
        }
    }

    #[test]
    fn validate_clean_tree() {
        assert!(bob_tree().validate().is_ok());
    }

    #[test]
    fn validate_missing_entry() {
        let mut t = bob_tree();
        t.entry = "nope".into();
        assert!(matches!(t.validate(), Err(crate::DialogueError::MissingEntry(_))));
    }

    #[test]
    fn validate_dangling_next() {
        let mut t = bob_tree();
        // Replace intro's next with a non-existent node.
        let mut intro = t.nodes.get("intro").unwrap().clone();
        intro.next = Some("ghost".into());
        t.nodes.insert("intro".into(), intro);
        match t.validate() {
            Err(crate::DialogueError::DanglingRef { from, .. }) => assert_eq!(from, "intro"),
            other => panic!("expected DanglingRef, got {other:?}"),
        }
    }

    #[test]
    fn ron_round_trip() {
        let t = bob_tree();
        let ron_str = t.to_ron().unwrap();
        let back = DialogTree::from_ron(&ron_str).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn load_bob_intro_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/dialogs/bob_intro.dialog.ron");
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let t = DialogTree::from_ron(&src).expect("parse fixture");
        t.validate().expect("validate fixture");
        assert_eq!(t.id.as_str(), "bob_intro");
        assert_eq!(t.entry.as_str(), "intro");
        assert!(t.get("ask").unwrap().is_branching());
    }
}
