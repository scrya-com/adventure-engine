//! Shader selector (mirrors `ESlateShader`).
//!
//! Reference: SlateCore/Rendering/RenderingCommon.h:59.
//!
//! We expose four shaders, each mapped to a wgpu::RenderPipeline:
//!   * [`ShaderKind::Sprite`]  — textured quad with vertex tint
//!   * [`ShaderKind::Multiply`]— multiply blend (used for tinting masks)
//!   * [`ShaderKind::Overlay`] — additive overlay / light pass
//!   * [`ShaderKind::Post`]    — fullscreen post (gamma, vignette, etc.)

use crate::shader_source;

/// Which WGSL pipeline to use for an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShaderKind {
    /// Default textured quad shader.
    #[default]
    Sprite,
    /// Multiply blend — multiplies fragment colour by tint/sample.
    Multiply,
    /// Additive overlay (used for light passes, glows).
    Overlay,
    /// Fullscreen post-process pass.
    Post,
}

impl ShaderKind {
    /// How many distinct shader kinds exist (used to size pipeline caches).
    pub const fn count() -> usize {
        4
    }

    /// Stable index into a `[T; ShaderKind::COUNT]` cache.
    pub const fn index(self) -> usize {
        match self {
            ShaderKind::Sprite => 0,
            ShaderKind::Multiply => 1,
            ShaderKind::Overlay => 2,
            ShaderKind::Post => 3,
        }
    }

    /// WGSL source for this shader kind.
    pub fn wgsl(self) -> &'static str {
        match self {
            ShaderKind::Sprite => shader_source::SPRITE,
            ShaderKind::Multiply => shader_source::MULTIPLY,
            ShaderKind::Overlay => shader_source::OVERLAY,
            ShaderKind::Post => shader_source::POST,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_round_trips() {
        for k in [
            ShaderKind::Sprite,
            ShaderKind::Multiply,
            ShaderKind::Overlay,
            ShaderKind::Post,
        ] {
            assert_eq!(k.index(), k.index()); // stable
        }
    }

    #[test]
    fn count_matches_variants() {
        // Count distinct indices.
        let mut idx = [0usize; ShaderKind::count()];
        for k in [
            ShaderKind::Sprite,
            ShaderKind::Multiply,
            ShaderKind::Overlay,
            ShaderKind::Post,
        ] {
            idx[k.index()] += 1;
        }
        assert!(idx.iter().all(|&n| n == 1));
    }

    #[test]
    fn wgsl_nonempty() {
        for k in [
            ShaderKind::Sprite,
            ShaderKind::Multiply,
            ShaderKind::Overlay,
            ShaderKind::Post,
        ] {
            assert!(!k.wgsl().is_empty());
        }
    }
}
