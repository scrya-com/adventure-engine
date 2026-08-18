//! Asset pipeline: manifest, async loader, reference tracking, and
//! Ren'Py-name → filesystem resolution for HarmonyHaven-style packs.
//!
//! Models UE's [`FAssetData`](AssetRegistry/AssetData.h) + async loader +
//! soft/hard pointer distinction. See `docs/DATA-FORMATS.md`.

#![deny(missing_docs)]

use std::path::Path;

use image::GenericImageView;

pub mod resolve;

pub use resolve::{
    is_movie_path, parse_visuals_rpy, resolve_audio, resolve_visual, AssetResolver, AUDIO_EXTS,
    DEFAULT_EXTRACT, VISUAL_EXTS,
};

/// Failed to decode an image file.
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    /// `image` crate decode failure.
    #[error("decode {path}: {source}")]
    Decode {
        /// File that failed.
        path: String,
        /// Underlying error.
        #[source]
        source: image::ImageError,
    },
}

/// Decode an image file to tightly packed RGBA8.
pub fn load_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>), ImageError> {
    let img = image::open(path).map_err(|source| ImageError::Decode {
        path: path.display().to_string(),
        source,
    })?;
    let (w, h) = img.dimensions();
    Ok((w, h, img.to_rgba8().into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_harmonyhaven_jpg_when_present() {
        let p = std::path::PathBuf::from(DEFAULT_EXTRACT).join("images/bgn1.jpg");
        if !p.is_file() {
            return;
        }
        let (w, h, rgba) = load_rgba(&p).expect("decode bgn1.jpg");
        assert!(w > 0 && h > 0);
        assert_eq!(rgba.len(), (w * h * 4) as usize);
    }
}
