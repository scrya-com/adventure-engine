//! Rhai scripting wrapper — conditions + side effects for dialog + state.
//!
//! Adventure scripts are deliberately tiny: conditions are expressions
//! that read state, side effects are statements that write state. We
//! expose a small set of helpers (`has_tag`, `add_tag`, `remove_tag`,
//! `set_int/float/bool/str`) plus plain variable references.
//!
//! Reference design: UE's Blueprint VM, but a fraction of the surface.
//! See `docs/SCRIPTING.md` for the 80/20 data-driven / Rhai split.

#![deny(missing_docs)]

pub mod error;
pub mod host;

pub use error::ScriptError;
pub use host::ScriptHost;
