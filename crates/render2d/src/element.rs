//! `DrawElement` — a single authored draw call (mirrors `FSlateDrawElement`).
//!
//! Reference: SlateCore/Rendering/DrawElementTypes.h:45.
//!
//! Elements are the input to [`crate::ElementBatcher`]. They are *records*,
//! not GPU resources — the batcher groups them by sort key, the renderer
//! then uploads + draws each group.

use adventure_core::math::Vec2;

use crate::effect::DrawEffect;
use crate::shader::ShaderKind;

/// Stable handle to a texture in the renderer's texture table.
///
/// Zero is the "no texture" sentinel — used by flat-colour elements
/// which the renderer services from a 1×1 default texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextureId(pub u32);

impl TextureId {
    /// Sentinel: no texture bound.
    pub const NONE: TextureId = TextureId(0);
    /// First usable texture id (the renderer reserves 0 for its white texel).
    pub const FIRST: TextureId = TextureId(1);
}

/// UV rectangle in texture-normalized coords.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UvRect {
    /// Top-left corner.
    pub min: Vec2,
    /// Bottom-right corner.
    pub max: Vec2,
}

impl UvRect {
    /// Full 0..1 quad.
    pub const FULL: UvRect = UvRect {
        min: Vec2 {
            x: 0.0,
            y: 0.0,
        },
        max: Vec2 { x: 1.0, y: 1.0 },
    };
}

/// Vertex tint (linear RGBA). White = no tinting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tint(pub glam::Vec4);

impl Default for Tint {
    fn default() -> Self {
        Tint(glam::Vec4::ONE)
    }
}

impl Tint {
    /// White (no-op tint).
    pub const IDENTITY: Tint = Tint(glam::Vec4::ONE);
    /// Construct from linear RGBA.
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Tint(glam::Vec4::new(r, g, b, a))
    }
}

/// A single authored draw element.
///
/// Layer is the highest-level sort key; elements at higher layer draw
/// *later* (on top). Within a layer, elements are batched by
/// (shader, texture, effect) so the GPU can avoid pipeline switches.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawElement {
    /// Render layer (UE's `Layer` arg on `FSlateDrawElement::MakeTime`).
    pub layer: i32,
    /// Shader pipeline for this element.
    pub shader: ShaderKind,
    /// Effect bitset (participates in the sort key).
    pub effect: DrawEffect,
    /// Texture handle (`TextureId::NONE` for flat-colour).
    pub texture: TextureId,
    /// Source UV rectangle (sub-rect within the texture).
    pub uv: UvRect,
    /// Vertex tint (multiplied in-shader).
    pub tint: Tint,
    /// Triangle list vertex positions in pixel space (after view_proj).
    pub positions: Vec<Vec2>,
    /// UVs aligned with `positions`.
    pub uvs: Vec<Vec2>,
}

impl DrawElement {
    /// Build the sort key for this element.
    pub fn sort_key(&self) -> SortKey {
        SortKey {
            layer: self.layer,
            shader: self.shader.index() as u8,
            effect: self.effect.bits(),
            texture: self.texture.0,
        }
    }
}

/// Sort key — packed tuple of `(layer, shader, effect, texture)`.
///
/// Two elements with the same key may be merged into a single batch
/// regardless of authoring order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SortKey {
    /// Highest-order sort: layer.
    pub layer: i32,
    /// Shader index (0..4).
    pub shader: u8,
    /// Effect bitset.
    pub effect: u32,
    /// Texture handle.
    pub texture: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(layer: i32, shader: ShaderKind, tex: TextureId, effect: DrawEffect) -> DrawElement {
        DrawElement {
            layer,
            shader,
            effect,
            texture: tex,
            uv: UvRect::FULL,
            tint: Tint::IDENTITY,
            positions: vec![Vec2::ZERO, Vec2::ZERO, Vec2::ZERO],
            uvs: vec![Vec2::ZERO, Vec2::ZERO, Vec2::ZERO],
        }
    }

    #[test]
    fn sort_order_layers() {
        let mut ks = [
            e(5, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE).sort_key(),
            e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE).sort_key(),
            e(3, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE).sort_key(),
        ];
        ks.sort();
        assert_eq!(ks[0].layer, 0);
        assert_eq!(ks[1].layer, 3);
        assert_eq!(ks[2].layer, 5);
    }

    #[test]
    fn sort_order_within_layer_by_shader() {
        let s1 = e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE).sort_key();
        let s2 = e(0, ShaderKind::Post, TextureId::FIRST, DrawEffect::NONE).sort_key();
        assert!(s1 < s2);
    }

    #[test]
    fn sort_order_within_layer_by_effect() {
        let a = e(
            0,
            ShaderKind::Sprite,
            TextureId::FIRST,
            DrawEffect::NONE,
        )
        .sort_key();
        let b = e(
            0,
            ShaderKind::Sprite,
            TextureId::FIRST,
            DrawEffect::TINTED,
        )
        .sort_key();
        assert!(a < b);
    }

    #[test]
    fn sort_order_within_layer_by_texture() {
        let a = e(0, ShaderKind::Sprite, TextureId(2), DrawEffect::NONE).sort_key();
        let b = e(0, ShaderKind::Sprite, TextureId(5), DrawEffect::NONE).sort_key();
        assert!(a < b);
    }

    #[test]
    fn equal_keys_equal() {
        let a = e(
            2,
            ShaderKind::Sprite,
            TextureId(7),
            DrawEffect::TINTED | DrawEffect::NO_BLENDING,
        )
        .sort_key();
        let b = e(
            2,
            ShaderKind::Sprite,
            TextureId(7),
            DrawEffect::TINTED | DrawEffect::NO_BLENDING,
        )
        .sort_key();
        assert_eq!(a, b);
    }

    #[test]
    fn mat4_default_is_identity() {
        // Sanity: glam Mat4::IDENTITY is the orthographic-friendly default.
        let m = glam::Mat4::IDENTITY;
        let p = m * glam::Vec4::new(1.0, 2.0, 0.0, 1.0);
        assert_eq!(p, glam::Vec4::new(1.0, 2.0, 0.0, 1.0));
    }
}
