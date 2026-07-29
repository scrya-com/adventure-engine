//! Errors from the dialog subsystem.

use thiserror::Error;

/// Errors from dialog parsing + runner.
#[derive(Debug, Error)]
pub enum DialogueError {
    /// RON parse failure.
    #[error("dialog ron parse: {0}")]
    Ron(String),
    /// RON serialize failure.
    #[error("dialog ron serialize: {0}")]
    Serialize(String),
    /// Tree references an entry id that doesn't exist.
    #[error("entry node {0} does not exist in tree")]
    MissingEntry(String),
    /// A `next` / `Choice::next` pointed at a node that doesn't exist.
    #[error("dangling reference from {from} to {to}")]
    DanglingRef {
        /// Node that contained the dangling reference.
        from: String,
        /// Id that should have existed but didn't.
        to: String,
    },
    /// Tried to advance / choose after the conversation finished.
    #[error("dialog has finished")]
    Finished,
    /// Tried to advance / choose before `start`.
    #[error("dialog has not started")]
    NotStarted,
    /// Tried to use `advance` on a branching node.
    #[error("cannot advance a branching node — use choose() instead")]
    Branching,
    /// Tried to use `choose` on a linear node.
    #[error("cannot choose on a linear node — use advance() instead")]
    Linear,
    /// Tried to advance from a terminal node (no next).
    #[error("node has no next")]
    NoNext,
    /// Choice index out of range.
    #[error("choice index {index} out of range (len {len})")]
    ChoiceOutOfRange {
        /// Index the caller passed.
        index: usize,
        /// Number of choices on the node.
        len: usize,
    },
    /// Choice exists but its condition is currently false (or failed closed).
    #[error("choice {index} is not available under current conditions")]
    ChoiceUnavailable {
        /// Index the caller passed.
        index: usize,
    },
    /// Skipped too many nodes with false `condition` (possible cycle).
    #[error("condition skip limit exceeded while entering dialog nodes")]
    ConditionSkipLimit,
    /// A Rhai script (`on_enter`, side effects, condition) failed.
    #[error("script: {0}")]
    Script(#[from] adventure_scripting::ScriptError),
}
