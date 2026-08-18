//! Present a looping movie as a fullscreen wgpu texture.

use std::path::Path;
use std::time::Instant;

use adventure_core::math::Vec2;
use adventure_render2d::{
    DrawEffect, DrawElement, ElementBatcher, ShaderKind, TextureId, Tint, UvRect, WgpuRenderer,
};

use crate::decoder::{MovieDecoder, MovieError};

/// Owns a [`MovieDecoder`] and the wgpu texture it uploads into.
pub struct MoviePlayer {
    decoder: MovieDecoder,
    texture: TextureId,
}

impl MoviePlayer {
    /// Start decoding `path` and upload the first frame.
    ///
    /// Fails if ffmpeg cannot open the file — callers must not log-and-skip.
    pub fn start(
        renderer: &mut WgpuRenderer,
        path: impl AsRef<Path>,
        loop_: bool,
    ) -> Result<Self, MovieError> {
        let mut decoder = MovieDecoder::open(path, loop_)?;
        let display = decoder.path().display().to_string();
        let (width, height) = (decoder.width(), decoder.height());
        let frame = match decoder.read_frame()? {
            Some(bytes) => bytes.to_vec(),
            None => {
                return Err(MovieError::Decode {
                    path: display,
                    detail: "no frames decoded".into(),
                });
            }
        };
        let texture = renderer
            .upload_texture(width, height, &frame)
            .map_err(|e| MovieError::Decode {
                path: display,
                detail: e.to_string(),
            })?;
        Ok(Self { decoder, texture })
    }

    /// Looping [`MoviePlayer::start`].
    pub fn start_looping(
        renderer: &mut WgpuRenderer,
        path: impl AsRef<Path>,
    ) -> Result<Self, MovieError> {
        Self::start(renderer, path, true)
    }

    /// Texture currently holding the latest frame.
    pub fn texture(&self) -> TextureId {
        self.texture
    }

    /// Native pixel size.
    pub fn size(&self) -> (u32, u32) {
        (self.decoder.width(), self.decoder.height())
    }

    /// Whether the decoder loops.
    pub fn loops(&self) -> bool {
        self.decoder.loops()
    }

    /// Pull a time-paced frame and upload it. Returns `true` if the texture changed.
    pub fn tick(&mut self, renderer: &mut WgpuRenderer, now: Instant) -> Result<bool, MovieError> {
        match self.decoder.poll_frame(now)? {
            Some(frame) => {
                renderer
                    .update_texture(self.texture, frame)
                    .map_err(|e| MovieError::Decode {
                        path: self.decoder.path().display().to_string(),
                        detail: e.to_string(),
                    })?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Fullscreen quad covering the window (Ren'Py `Movie()` as a scene).
    pub fn fullscreen_element(&self, width: f32, height: f32) -> DrawElement {
        fullscreen_quad(self.texture, width, height)
    }

    /// Push a fullscreen movie quad into the batcher.
    pub fn push_fullscreen(&self, batcher: &mut ElementBatcher, width: f32, height: f32) {
        batcher.push(self.fullscreen_element(width, height));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::encode_color_webm;

    #[test]
    fn uploads_decoded_frame_to_wgpu_when_adapter_exists() {
        let dir = std::env::temp_dir().join(format!(
            "ae-movie-gpu-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("red.webm");
        encode_color_webm(&path, 64, 48, 4, 10).expect("encode");

        let renderer = pollster::block_on(WgpuRenderer::new_headless());
        let Ok(mut renderer) = renderer else {
            // CI / machines without a GPU adapter — decode path is covered
            // in decoder tests; skip the upload half.
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        let mut player = MoviePlayer::start_looping(&mut renderer, &path).expect("start movie");
        assert_eq!(player.size(), (64, 48));
        assert_ne!(player.texture(), TextureId::NONE);
        let _ = player.tick(&mut renderer, Instant::now());
        let el = player.fullscreen_element(1280.0, 720.0);
        assert_eq!(el.positions.len(), 6);
        assert_eq!(el.texture, player.texture());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Two-triangle fullscreen sprite covering `[0, width] × [0, height]`.
pub fn fullscreen_quad(texture: TextureId, width: f32, height: f32) -> DrawElement {
    DrawElement {
        layer: 0,
        shader: ShaderKind::Sprite,
        effect: DrawEffect::NONE,
        texture,
        uv: UvRect::FULL,
        tint: Tint::IDENTITY,
        positions: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(width, 0.0),
            Vec2::new(0.0, height),
            Vec2::new(0.0, height),
            Vec2::new(width, 0.0),
            Vec2::new(width, height),
        ],
        uvs: vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(0.0, 1.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
        ],
    }
}
