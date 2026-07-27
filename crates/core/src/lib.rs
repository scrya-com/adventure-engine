//! Foundation primitives for the adventure engine.
//!
//! This crate is the lowest layer of the engine — it depends on no other
//! adventure crate and provides:
//! - [`AssetId`] — opaque path-hash identifier for all asset references
//! - [`Error`] and [`Result`] — unified error type for the engine
//! - Math re-exports from [`glam`]
//! - Logging helpers via [`tracing`]
//! - Interning helpers via [`smol_str`]
//!
//! See `docs/ARCHITECTURE.md` for where this crate sits in the dependency graph.

#![deny(missing_docs)]

use std::fmt;

/// Re-export of [`glam`] math types.
///
/// All engine crates use this rather than depending on glam directly, so we
/// can swap or wrap math types centrally.
pub mod math {
    pub use glam::{Affine2, Mat2, Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
}

/// Re-export of common tracing macros.
pub use tracing::{debug, error, info, span, trace, warn};

/// A small inline-able string used for tags, names, identifiers.
pub use smol_str::SmolStr;

/// Opaque asset identifier.
///
/// Wraps a 64-bit hash of the asset's path (e.g. `"bg/clearing"`,
/// `"sprites/walk_n"`). The hash is deterministic across runs and platforms
/// so asset references can be stored in save files.
///
/// Create with [`AssetId::from_path`] or via `From<&str>` / `From<String>`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(u64);

impl AssetId {
    /// Construct an [`AssetId`] from a path string.
    ///
    /// Uses fxhash-style mixing on the UTF-8 bytes. The same path always
    /// produces the same id.
    pub fn from_path(path: &str) -> Self {
        // Simple FNV-1a 64-bit — good distribution for short strings,
        // deterministic across platforms, no deps.
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in path.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Self(hash)
    }

    /// Raw u64 value (for serialization).
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Construct from a raw u64 (for deserialization).
    pub fn from_raw(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Debug for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AssetId({:#018x})", self.0)
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}

impl From<&str> for AssetId {
    fn from(path: &str) -> Self {
        Self::from_path(path)
    }
}

impl From<&String> for AssetId {
    fn from(path: &String) -> Self {
        Self::from_path(path)
    }
}

impl serde::Serialize for AssetId {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> std::result::Result<S::Ok, S::Error> {
        ser.serialize_u64(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for AssetId {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> std::result::Result<Self, D::Error> {
        Ok(Self(u64::deserialize(de)?))
    }
}

/// Unified error type for the adventure engine.
///
/// Subsystems convert their internal errors into one of these variants via
/// `#[from]` so callers get a single [`Result`] type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// I/O error (file system, network — though we have no network).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    /// Asset not found, malformed, or otherwise unusable.
    #[error("asset: {0}")]
    Asset(String),

    /// Save game error (version, migration, corruption).
    #[error("save: {0}")]
    Save(String),

    /// Scripting error (Rhai compile / runtime).
    #[error("script: {0}")]
    Script(String),

    /// Render device error.
    #[error("render: {0}")]
    Render(String),

    /// Audio device error.
    #[error("audio: {0}")]
    Audio(String),

    /// Catch-all for errors that don't fit elsewhere.
    #[error("{0}")]
    Other(String),
}

/// Result alias used across the engine.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_id_is_deterministic() {
        let a = AssetId::from_path("bg/clearing");
        let b = AssetId::from_path("bg/clearing");
        assert_eq!(a, b);
    }

    #[test]
    fn asset_id_is_path_sensitive() {
        let a = AssetId::from_path("bg/clearing");
        let b = AssetId::from_path("bg/clearing2");
        assert_ne!(a, b);
    }

    #[test]
    fn asset_id_debug_format() {
        let id = AssetId::from_path("test");
        assert!(format!("{id:?}").starts_with("AssetId(0x"));
    }
}
