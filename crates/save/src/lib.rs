//! Save game format with versioned header and migrations.
//!
//! Mirrors UE's [`FSaveGameHeader`](Engine/Private/GameplayStatics.cpp:89):
//! magic + engine version + custom versions per subsystem. See
//! `docs/SAVE.md` for the full design.

#![deny(missing_docs)]

/// Magic bytes identifying an adventure save file. ASCII `'SAVG'` as
/// little-endian u32 — same value UE uses.
pub const SAVE_MAGIC: u32 = 0x53415647;

/// Schema name. Bumped only on incompatible top-level layout changes.
pub const SCHEMA_NAME: &str = "adventure-save-v1";
