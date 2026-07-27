//! Asset pipeline: manifest, async loader, reference tracking.
//!
//! Models UE's [`FAssetData`](AssetRegistry/AssetData.h) + async loader +
//! soft/hard pointer distinction. See `docs/DATA-FORMATS.md`.

#![deny(missing_docs)]
