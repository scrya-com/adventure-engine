//! Errors from the scripting host.

use thiserror::Error;

/// Errors from Rhai evaluation.
#[derive(Debug, Error)]
pub enum ScriptError {
    /// Rhai parse or runtime error.
    #[error("rhai: {0}")]
    Rhai(String),
    /// The expression did not evaluate to a bool.
    #[error("condition did not evaluate to bool: {0}")]
    NotBool(String),
    /// The condition expression was empty or whitespace.
    #[error("empty condition")]
    Empty,
}

impl From<rhai::EvalAltResult> for ScriptError {
    fn from(e: rhai::EvalAltResult) -> Self {
        ScriptError::Rhai(e.to_string())
    }
}

impl From<rhai::ParseError> for ScriptError {
    fn from(e: rhai::ParseError) -> Self {
        ScriptError::Rhai(e.to_string())
    }
}

impl From<rhai::ParseErrorType> for ScriptError {
    fn from(e: rhai::ParseErrorType) -> Self {
        ScriptError::Rhai(e.to_string())
    }
}
