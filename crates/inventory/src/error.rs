//! Errors from the inventory subsystem.

use thiserror::Error;

/// Errors from item parsing, inventory ops, and combine resolution.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryError {
    /// RON parse failure.
    #[error("item ron parse: {0}")]
    Ron(String),
    /// RON serialize failure.
    #[error("item ron serialize: {0}")]
    Serialize(String),
    /// Inventory is at capacity.
    #[error("inventory full (capacity {capacity})")]
    Full {
        /// Maximum number of distinct slots.
        capacity: usize,
    },
    /// Tried to remove more of an item than is held.
    #[error("not enough of item `{id}`: have {have}, need {need}")]
    NotEnough {
        /// Item id.
        id: String,
        /// Count currently held.
        have: u32,
        /// Count requested.
        need: u32,
    },
    /// Item definition missing from the catalog.
    #[error("unknown item `{0}`")]
    UnknownItem(String),
    /// Combine rule table is empty / no matching recipe (fail closed).
    #[error("cannot combine `{a}` with `{b}`")]
    CannotCombine {
        /// First item id (as requested).
        a: String,
        /// Second item id (as requested).
        b: String,
    },
    /// Verb not available on this item / target.
    #[error("verb `{verb}` is not available")]
    VerbUnavailable {
        /// Verb kind label.
        verb: String,
    },
}
