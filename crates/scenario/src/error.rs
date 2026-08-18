//! Errors from the scenario (story-script) subsystem.

use thiserror::Error;

/// Errors from story parsing, compilation, validation, and running.
#[derive(Debug, Error)]
pub enum ScenarioError {
    /// RON parse failure.
    #[error("story ron parse: {0}")]
    Ron(String),
    /// RON serialize failure.
    #[error("story ron serialize: {0}")]
    Serialize(String),
    /// Story failed validation (all issues collected).
    #[error("story validation failed:\n{}", .0.join("\n"))]
    Validation(Vec<String>),
    /// Runtime transfer to an unknown label (validation should have caught it).
    #[error("dangling label {0}")]
    DanglingLabel(String),
    /// Tried to advance / choose after the story finished.
    #[error("story has finished")]
    Finished,
    /// Tried to advance / choose before `start`.
    #[error("story has not started")]
    NotStarted,
    /// `advance` while blocked on a menu.
    #[error("blocked on a menu — use choose() instead")]
    BlockedAtMenu,
    /// `choose` while not blocked on a menu.
    #[error("not blocked on a menu — use advance() instead")]
    NotBlockedAtMenu,
    /// `advance` / `choose` while blocked on text input.
    #[error("blocked on input — use submit_text() instead")]
    BlockedAtInput,
    /// `submit_text` while not blocked on input.
    #[error("not blocked on input — use advance() / choose() instead")]
    NotBlockedAtInput,
    /// Choice index out of range.
    #[error("choice index {index} out of range (len {len})")]
    ChoiceOutOfRange {
        /// Index the caller passed.
        index: usize,
        /// Number of authored choices.
        len: usize,
    },
    /// Choice condition currently false (or failed closed).
    #[error("choice {index} is not available under current conditions")]
    ChoiceUnavailable {
        /// Index the caller passed.
        index: usize,
    },
    /// A Rhai script (condition, effects, `Exec`) failed.
    #[error("script: {0}")]
    Script(#[from] adventure_scripting::ScriptError),
}
