//! Rhai scripting wrapper.
//!
//! 80/20 split: 80% of adventure logic is pure data (RON), 20% needs
//! arithmetic / conditions / side effects. Rhai is the 20% — Rust-native,
//! sandboxed, no GC, no FFI. See `docs/SCRIPTING.md`.

#![deny(missing_docs)]
