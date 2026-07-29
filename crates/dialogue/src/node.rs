//! Dialog node + choice data types.
//!
//! A dialog tree is a directed graph of [`DialogNode`]s. Each node has
//! a speaker + line of text, an optional set of side effects to fire
//! when the node is entered, and either a single linear `next` hop or
//! a list of [`Choice`]s the player picks from.
//!
//! Both nodes and choices can carry a Rhai `condition` that gates
//! visibility — the runner hides options whose condition is false.

use adventure_core::SmolStr;
use serde::{Deserialize, Serialize};

/// Stable identifier for a node within a tree (e.g. `"intro"`).
pub type NodeId = SmolStr;

/// One player-selectable choice on a node.
///
/// Choices are visible only when their `condition` (if present)
/// evaluates true against the current game state. Selecting a choice
/// fires its `side_effects` (if present) and advances the dialog
/// to `next` (or ends the conversation if `None`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice {
    /// Player-facing choice text (e.g. `"What's your name?"`).
    pub text: SmolStr,
    /// Where to go next, or `None` to end the conversation.
    #[serde(default)]
    pub next: Option<NodeId>,
    /// Optional Rhai expression (e.g. `has_tag("State.NPC.Bob.Met")`).
    #[serde(default)]
    pub condition: Option<SmolStr>,
    /// Optional Rhai statements (e.g. `add_tag("..."); set_int("...", 1);`).
    #[serde(default)]
    pub side_effects: Option<SmolStr>,
}

impl Choice {
    /// Build a simple choice with text + next, no condition or side effects.
    pub fn new<S: Into<SmolStr>>(text: impl Into<SmolStr>, next: Option<S>) -> Self {
        Self {
            text: text.into(),
            next: next.map(Into::into),
            condition: None,
            side_effects: None,
        }
    }

    /// Attach a Rhai condition to this choice.
    pub fn with_condition(mut self, cond: impl Into<SmolStr>) -> Self {
        self.condition = Some(cond.into());
        self
    }

    /// Attach Rhai side effects to this choice.
    pub fn with_side_effects(mut self, fx: impl Into<SmolStr>) -> Self {
        self.side_effects = Some(fx.into());
        self
    }
}

/// A single dialog node: speaker + line + outgoing edges.
///
/// A node is either:
///   * **linear** — `next` is `Some(_)` and `choices` is empty
///     (player clicks "Continue").
///   * **branching** — `choices` is non-empty (player picks one).
///   * **terminal** — both `next` is `None` and `choices` is empty
///     (end of conversation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogNode {
    /// Stable id within this tree (referenced by `next` and `Choice::next`).
    pub id: NodeId,
    /// Speaker name (e.g. `"Bob"`).
    pub speaker: SmolStr,
    /// The line of dialog text.
    pub text: SmolStr,
    /// Linear next hop (used when `choices` is empty).
    #[serde(default)]
    pub next: Option<NodeId>,
    /// Branching choices (when present, takes priority over `next`).
    #[serde(default)]
    pub choices: Vec<Choice>,
    /// Side effects to fire when this node is entered.
    #[serde(default)]
    pub on_enter: Option<SmolStr>,
    /// Condition evaluated when *this* node is referenced from a `next` or
    /// `Choice::next` hop. If false, the runner skips to the target's own
    /// `next` (used for conditional inserts).
    #[serde(default)]
    pub condition: Option<SmolStr>,
}

impl DialogNode {
    /// Build a linear node that goes to `next`.
    pub fn linear<S: Into<SmolStr>>(
        id: impl Into<SmolStr>,
        speaker: impl Into<SmolStr>,
        text: impl Into<SmolStr>,
        next: Option<S>,
    ) -> Self {
        Self {
            id: id.into(),
            speaker: speaker.into(),
            text: text.into(),
            next: next.map(Into::into),
            choices: Vec::new(),
            on_enter: None,
            condition: None,
        }
    }

    /// Build a branching node with the given choices.
    pub fn branching(
        id: impl Into<SmolStr>,
        speaker: impl Into<SmolStr>,
        text: impl Into<SmolStr>,
        choices: Vec<Choice>,
    ) -> Self {
        Self {
            id: id.into(),
            speaker: speaker.into(),
            text: text.into(),
            next: None,
            choices,
            on_enter: None,
            condition: None,
        }
    }

    /// Attach `on_enter` side effects.
    pub fn with_on_enter(mut self, fx: impl Into<SmolStr>) -> Self {
        self.on_enter = Some(fx.into());
        self
    }

    /// Attach a Rhai condition evaluated when this node is entered.
    /// If false, the runner skips to this node's linear `next` (or finishes).
    pub fn with_condition(mut self, cond: impl Into<SmolStr>) -> Self {
        self.condition = Some(cond.into());
        self
    }

    /// Is this node terminal (no outgoing edges)?
    pub fn is_terminal(&self) -> bool {
        self.next.is_none() && self.choices.is_empty()
    }

    /// Is this node branching (player must pick a choice)?
    pub fn is_branching(&self) -> bool {
        !self.choices.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_node_round_trip() {
        let n = DialogNode::linear("intro", "Bob", "Hello there.", Some("ask_name"));
        let ron_str = ron::ser::to_string_pretty(&n, ron::ser::PrettyConfig::default()).unwrap();
        let back: DialogNode = ron::from_str(&ron_str).unwrap();
        assert_eq!(n, back);
    }

    #[test]
    fn branching_node_with_choice() {
        let n = DialogNode::branching(
            "ask",
            "Bob",
            "What do you want?",
            vec![
                Choice::new("Money", Some("give_money")),
                Choice::new("Nothing", None::<&str>).with_condition("has_tag(\"State.NPC.Bob.Met\")"),
            ],
        );
        assert!(n.is_branching());
        assert!(!n.is_terminal());
        assert_eq!(n.choices.len(), 2);
    }

    #[test]
    fn terminal_node() {
        let n = DialogNode::linear("bye", "Bob", "Goodbye.", None::<&str>);
        assert!(n.is_terminal());
        assert!(!n.is_branching());
    }
}
