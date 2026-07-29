//! [`DialogRunner`] — push-button state machine over a [`DialogTree`].
//!
//! The runner is a small state object that knows:
//!   * which tree it's running,
//!   * which node the player is currently viewing,
//!   * whether the conversation has finished.
//!
//! It delegates condition evaluation + side effects to [`ScriptHost`],
//! so the dialog data never touches state directly.

use adventure_scripting::ScriptHost;
use adventure_state::{Tags, VarTable};

use crate::node::{Choice, DialogNode, NodeId};
use crate::tree::DialogTree;

/// One visible choice on the current node (already filtered by condition).
#[derive(Clone, Debug, PartialEq)]
pub struct VisibleChoice {
    /// The index of this choice in the original `DialogNode::choices` list.
    /// Used by [`DialogRunner::choose`] to look up the original.
    pub source_index: usize,
    /// The choice text (player-facing).
    pub text: String,
}

/// State machine over a [`DialogTree`].
///
/// Lifecycle:
///   1. [`DialogRunner::start`] — enter the tree's entry node, fire `on_enter`,
///      land on the first visible position.
///   2. [`DialogRunner::current`] — read the current speaker/text + visible choices.
///   3. [`DialogRunner::choose`] — pick a branch (only valid when current
///      node is branching).
///   4. [`DialogRunner::advance`] — click "Continue" (only valid when
///      current node is linear).
///   5. Either of (3) / (4) lands on the next node and may set
///      [`DialogRunner::is_finished`] when arriving at a terminal node.
#[derive(Clone, Debug)]
pub struct DialogRunner {
    tree_id: String,
    /// Current node id, or `None` if the conversation has finished.
    current: Option<NodeId>,
    finished: bool,
    /// History of visited node ids (for debugging / "back" features).
    history: Vec<NodeId>,
}

impl DialogRunner {
    /// Build a fresh runner for the given tree. Does NOT enter the tree —
    /// call [`DialogRunner::start`] to fire `on_enter` on the entry node.
    pub fn new(tree: &DialogTree) -> Self {
        Self {
            tree_id: tree.id.to_string(),
            current: None,
            finished: false,
            history: Vec::new(),
        }
    }

    /// The tree id this runner was built for.
    pub fn tree_id(&self) -> &str {
        &self.tree_id
    }

    /// Has the conversation ended?
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Id of the current node, or `None` if finished / not yet started.
    pub fn current_id(&self) -> Option<&str> {
        self.current.as_deref()
    }

    /// Visited node ids in order (most-recent-last).
    pub fn history(&self) -> &[NodeId] {
        &self.history
    }

    /// Enter the tree's entry node. Fires `on_enter` on the entry node.
    ///
    /// Passing `&mut` to vars / tags allows side effects to mutate them.
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::Script`] if a side-effect fails.
    pub fn start(
        &mut self,
        tree: &DialogTree,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<(), crate::DialogueError> {
        let entry = tree.entry.clone();
        self.enter(tree, &entry, host, vars, tags)
    }

    /// Enter a specific node directly (skips the tree's `entry`).
    ///
    /// Useful for testing and for in-progress tree edits. Fires the
    /// target node's `on_enter`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::DanglingRef`] if the id doesn't
    /// resolve, or [`crate::DialogueError::Script`] on side-effect failure.
    pub fn start_at(
        &mut self,
        tree: &DialogTree,
        id: &NodeId,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<(), crate::DialogueError> {
        self.enter(tree, id, host, vars, tags)
    }

    /// Look up the current node in the tree.
    pub fn current<'a>(&self, tree: &'a DialogTree) -> Option<&'a DialogNode> {
        self.current.as_ref().and_then(|id| tree.get(id))
    }

    /// Compute the list of choices that are currently visible (i.e. their
    /// conditions evaluate true). Returns an empty vec when the current
    /// node is linear or terminal.
    ///
    /// Snapshot only — does not mutate state. Condition **script errors**
    /// fail closed (choice hidden) so a broken expression never unlocks
    /// a gated option in the UI. Use [`DialogRunner::choose`] for
    /// fail-closed API enforcement with a proper error.
    pub fn visible_choices(
        &self,
        tree: &DialogTree,
        host: &ScriptHost,
        vars: &VarTable,
        tags: &Tags,
    ) -> Vec<VisibleChoice> {
        let Some(node) = self.current(tree) else {
            return Vec::new();
        };
        node.choices
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let ok = match c.condition.as_deref() {
                    // Fail closed on script errors — hide the choice.
                    Some(expr) => host.eval_condition(expr, vars, tags).unwrap_or(false),
                    None => true,
                };
                if ok {
                    Some(VisibleChoice {
                        source_index: i,
                        text: c.text.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Pick the choice at `source_index` (the index in the original
    /// `DialogNode::choices` list — use [`VisibleChoice::source_index`]).
    ///
    /// Re-evaluates the choice condition (fail closed), fires
    /// `side_effects`, then advances to `next` (or finishes if `None`).
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::ChoiceOutOfRange`],
    /// [`crate::DialogueError::ChoiceUnavailable`] if the condition is
    /// false, or [`crate::DialogueError::Script`] on script failure.
    pub fn choose(
        &mut self,
        tree: &DialogTree,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
        source_index: usize,
    ) -> Result<(), crate::DialogueError> {
        if self.finished {
            return Err(crate::DialogueError::Finished);
        }
        let Some(node) = self.current(tree) else {
            return Err(crate::DialogueError::NotStarted);
        };
        if !node.is_branching() {
            return Err(crate::DialogueError::Linear);
        }
        let choice: &Choice = node.choices.get(source_index).ok_or_else(|| {
            crate::DialogueError::ChoiceOutOfRange {
                index: source_index,
                len: node.choices.len(),
            }
        })?;

        // Enforce condition at choose-time (not only at UI filter time).
        if let Some(expr) = choice.condition.as_deref() {
            let ok = host.eval_condition(expr, vars, tags)?;
            if !ok {
                return Err(crate::DialogueError::ChoiceUnavailable {
                    index: source_index,
                });
            }
        }

        // Clone next + side_effects so we can release the borrow on `node`.
        let side_effects = choice.side_effects.clone();
        let next = choice.next.clone();

        if let Some(fx) = side_effects.as_deref() {
            host.run(fx, vars, tags)?;
        }

        match next {
            Some(next) => self.enter(tree, &next, host, vars, tags),
            None => {
                self.finished = true;
                self.current = None;
                Ok(())
            }
        }
    }

    /// Advance from a linear node to its `next`. Errors if the current
    /// node is branching (use [`DialogRunner::choose`]) or has no `next`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::DialogueError::NoNext`] if the current node is
    /// terminal / has no linear next, or [`crate::DialogueError::Branching`]
    /// if the current node is branching.
    pub fn advance(
        &mut self,
        tree: &DialogTree,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<(), crate::DialogueError> {
        if self.finished {
            return Err(crate::DialogueError::Finished);
        }
        let Some(node) = self.current(tree) else {
            return Err(crate::DialogueError::NotStarted);
        };
        if node.is_branching() {
            return Err(crate::DialogueError::Branching);
        }
        match &node.next {
            Some(next) => self.enter(tree, next, host, vars, tags),
            None => {
                // Terminal linear node — conversation ends.
                self.finished = true;
                self.current = None;
                Ok(())
            }
        }
    }

    /// Max hops when skipping nodes whose `condition` is false.
    const MAX_CONDITION_SKIPS: usize = 32;

    /// Internal: enter a node. If the node's `condition` is present and
    /// false, skip to its linear `next` (or finish if terminal). Then
    /// records history, fires `on_enter`, updates `current`.
    fn enter(
        &mut self,
        tree: &DialogTree,
        id: &NodeId,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<(), crate::DialogueError> {
        let mut id = id.clone();
        let mut skips = 0;

        loop {
            let node = tree.get(&id).ok_or_else(|| {
                crate::DialogueError::DanglingRef {
                    from: self.current.as_deref().unwrap_or("<start>").to_string(),
                    to: id.to_string(),
                }
            })?;

            // Evaluate node-level gate (optional). Fail closed on script error.
            if let Some(expr) = node.condition.as_deref() {
                let ok = host.eval_condition(expr, vars, tags)?;
                if !ok {
                    skips += 1;
                    if skips > Self::MAX_CONDITION_SKIPS {
                        return Err(crate::DialogueError::ConditionSkipLimit);
                    }
                    // Skip to linear next, or finish if no next.
                    match &node.next {
                        Some(next) => {
                            id = next.clone();
                            continue;
                        }
                        None => {
                            self.finished = true;
                            self.current = None;
                            return Ok(());
                        }
                    }
                }
            }

            // Land here.
            self.history.push(id.clone());

            if let Some(fx) = node.on_enter.as_deref() {
                host.run(fx, vars, tags)?;
            }

            self.current = Some(id);
            self.finished = false;
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Choice;
    use adventure_state::Tag;
    use std::collections::BTreeMap;

    fn bob_tree() -> DialogTree {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "intro".into(),
            DialogNode::linear("intro", "Bob", "Hello.", Some("ask"))
                .with_on_enter("add_tag(\"State.NPC.Bob.Met\")"),
        );
        nodes.insert(
            "ask".into(),
            DialogNode::branching(
                "ask",
                "Bob",
                "What do you want?",
                vec![
                    Choice::new("Money", Some("give")).with_side_effects(
                        "set_int(\"greed\", 1)",
                    ),
                    Choice::new("Secret", Some("secret"))
                        .with_condition("has_tag(\"State.NPC.Bob.Met\")"),
                    Choice::new("Bye", None::<&str>),
                ],
            ),
        );
        nodes.insert(
            "give".into(),
            DialogNode::linear("give", "Bob", "Here you go.", None::<&str>),
        );
        nodes.insert(
            "secret".into(),
            DialogNode::linear("secret", "Bob", "The secret is: 42.", None::<&str>),
        );
        DialogTree {
            id: "bob".into(),
            entry: "intro".into(),
            nodes,
        }
    }

    #[test]
    fn start_fires_on_enter_and_lands_on_entry() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        assert_eq!(r.current_id(), Some("intro"));
        // on_enter fired add_tag("State.NPC.Bob.Met")
        assert!(tags.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
    }

    #[test]
    fn advance_linear_node_moves_to_next() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        assert_eq!(r.current_id(), Some("ask"));
    }

    #[test]
    fn visible_choices_filters_by_condition() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let v = VarTable::new();
        let mut tags = Tags::new();

        // Without State.NPC.Bob.Met — secret hidden. Start a clone that
        // doesn't fire on_enter so the tag stays absent.
        let mut tags_clean = tags.clone();
        let mut r = DialogRunner::new(&t);
        // Manually place at "ask" — bypassing the intro's on_enter.
        let ask_id: NodeId = "ask".into();
        r.start_at(&t, &ask_id, &host, &mut VarTable::new(), &mut tags_clean).unwrap();
        let visible = r.visible_choices(&t, &host, &v, &tags_clean);
        assert_eq!(visible.len(), 2);
        assert!(visible.iter().all(|c| c.text != "Secret"));

        // Now add the tag — secret visible.
        tags.add(Tag::new("State.NPC.Bob.Met").unwrap());
        let visible = r.visible_choices(&t, &host, &v, &tags);
        assert_eq!(visible.len(), 3);
        assert!(visible.iter().any(|c| c.text == "Secret"));
    }

    #[test]
    fn choose_runs_side_effects_and_advances() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        // Money is at index 0.
        r.choose(&t, &host, &mut v, &mut tags, 0).unwrap();
        assert_eq!(r.current_id(), Some("give"));
        match v.get("greed") {
            Some(adventure_state::VarValue::I(1)) => {}
            other => panic!("expected greed=I(1), got {other:?}"),
        }
    }

    #[test]
    fn choose_with_no_next_finishes_conversation() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        // "Bye" is at index 2.
        r.choose(&t, &host, &mut v, &mut tags, 2).unwrap();
        assert!(r.is_finished());
        assert_eq!(r.current_id(), None);
    }

    #[test]
    fn choose_out_of_range_errors() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        let err = r.choose(&t, &host, &mut v, &mut tags, 99).unwrap_err();
        assert!(matches!(err, crate::DialogueError::ChoiceOutOfRange { index: 99, .. }));
    }

    #[test]
    fn advance_on_branching_errors() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        let err = r.advance(&t, &host, &mut v, &mut tags).unwrap_err();
        assert!(matches!(err, crate::DialogueError::Branching));
    }

    #[test]
    fn history_records_visited_nodes() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        r.choose(&t, &host, &mut v, &mut tags, 0).unwrap();
        assert_eq!(
            r.history().iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            vec!["intro", "ask", "give"]
        );
    }

    #[test]
    fn choose_rejects_gated_choice() {
        let t = bob_tree();
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        // Land on ask without Met tag.
        let ask: NodeId = "ask".into();
        r.start_at(&t, &ask, &host, &mut v, &mut tags).unwrap();
        // Secret is index 1, gated by has_tag Met — should be unavailable.
        let err = r.choose(&t, &host, &mut v, &mut tags, 1).unwrap_err();
        assert!(matches!(
            err,
            crate::DialogueError::ChoiceUnavailable { index: 1 }
        ));
    }

    #[test]
    fn node_condition_skips_to_next() {
        // intro (always) → gated (false) → land (visible)
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "intro".into(),
            DialogNode::linear("intro", "N", "hi", Some("gated")),
        );
        nodes.insert(
            "gated".into(),
            DialogNode::linear("gated", "N", "skip me", Some("land"))
                .with_condition("false"),
        );
        nodes.insert(
            "land".into(),
            DialogNode::linear("land", "N", "made it", None::<&str>),
        );
        let t = DialogTree {
            id: "skip".into(),
            entry: "intro".into(),
            nodes,
        };
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut tags = Tags::new();
        let mut r = DialogRunner::new(&t);
        r.start(&t, &host, &mut v, &mut tags).unwrap();
        assert_eq!(r.current_id(), Some("intro"));
        r.advance(&t, &host, &mut v, &mut tags).unwrap();
        // Should have skipped "gated" and landed on "land".
        assert_eq!(r.current_id(), Some("land"));
        assert!(r.history().iter().any(|id| id.as_str() == "land"));
        // Skipped nodes are not recorded in history.
        assert!(!r.history().iter().any(|id| id.as_str() == "gated"));
    }
}
