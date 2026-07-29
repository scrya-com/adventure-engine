//! Dialog tree data model + runner.
//!
//! Reference design: UE's [`Common Conversation` plugin]
//! (Engine/Plugins/Experimental/CommonConversation/) — graph-driven,
//! tag-keyed, with entry/choice/link/requirement/side-effect/task nodes.
//!
//! Our flavor is smaller:
//!   * [`DialogNode`] — speaker + line + optional `next` (linear) or
//!     `choices` (branching), optional Rhai `on_enter` side effects
//!     and `condition`.
//!   * [`DialogTree`] — entry + node map, RON-roundtrippable.
//!   * [`DialogRunner`] — push-button state machine; delegates condition
//!     eval + side effects to [`adventure_scripting::ScriptHost`].

#![deny(missing_docs)]

pub mod error;
pub mod node;
pub mod runner;
pub mod tree;

pub use error::DialogueError;
pub use node::{Choice, DialogNode, NodeId};
pub use runner::{DialogRunner, VisibleChoice};
pub use tree::DialogTree;
