//! `ElementBatcher` — groups authored elements into drawable batches.
//!
//! Reference: SlateCore/Rendering/ElementBatcher.h:245-290.
//!
//! The batcher does **no** GPU work. It only sorts and groups elements
//! by [`SortKey`]. The renderer consumes the resulting list of batches
//! and turns each batch into a single `wgpu` draw call.

use std::mem;

use crate::element::{DrawElement, SortKey};

/// A single batch — a run of elements with the same sort key.
///
/// The renderer uploads `positions`/`uvs` (with the per-element `tint`
/// promoted to vertex attrib) and issues one draw call per batch.
#[derive(Debug, Clone, PartialEq)]
pub struct Batch {
    /// Sort key for this batch (shared by all source elements).
    pub key: SortKey,
    /// Per-element tint (parallel to the flattened vertex list).
    pub tints: Vec<glam::Vec4>,
    /// UV rect (per element; resolved into the per-vertex UVs).
    pub uv_rects: Vec<crate::element::UvRect>,
    /// Flattened triangle-list positions (pixel space).
    pub positions: Vec<adventure_core::math::Vec2>,
    /// Flattened triangle-list UVs (texture-normalized).
    pub uvs: Vec<adventure_core::math::Vec2>,
}

impl Batch {
    /// Number of vertices in this batch.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.vertex_count() / 3
    }
}

/// Stateless sorter + grouper.
#[derive(Debug, Default)]
pub struct ElementBatcher {
    elements: Vec<DrawElement>,
}

impl ElementBatcher {
    /// Empty batcher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push one element. Order within a sort-key bucket is preserved.
    pub fn push(&mut self, e: DrawElement) {
        self.elements.push(e);
    }

    /// Number of authored elements pending.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Whether the batcher has nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Reset between frames.
    pub fn clear(&mut self) {
        self.elements.clear();
    }

    /// Produce the sorted list of batches.
    ///
    /// Sorts by key, then merges adjacent equal-key elements into one batch.
    /// Per-element `tint` is fanned out to per-vertex during merge so the
    /// renderer can zip `positions`/`uvs`/`tints` 1:1.
    pub fn finish(&mut self) -> Vec<Batch> {
        // Stable sort by key — preserves authoring order within a key.
        self.elements.sort_by_key(|e| e.sort_key());

        let mut out: Vec<Batch> = Vec::new();
        for e in mem::take(&mut self.elements) {
            let n_verts = e.positions.len();
            // Try to append to the current batch if the key matches.
            let merge = matches!(out.last(), Some(b) if b.key == e.sort_key());
            if merge {
                let b = out.last_mut().unwrap();
                b.tints.extend(std::iter::repeat(e.tint.0).take(n_verts));
                b.uv_rects.push(e.uv);
                b.positions.extend(e.positions);
                b.uvs.extend(e.uvs);
            } else {
                out.push(Batch {
                    key: e.sort_key(),
                    tints: std::iter::repeat(e.tint.0).take(n_verts).collect(),
                    uv_rects: vec![e.uv],
                    positions: e.positions,
                    uvs: e.uvs,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::DrawEffect;
    use crate::element::{TextureId, Tint, UvRect};
    use crate::shader::ShaderKind;
    use adventure_core::math::Vec2;

    fn e(layer: i32, shader: ShaderKind, tex: TextureId, effect: DrawEffect) -> DrawElement {
        DrawElement {
            layer,
            shader,
            effect,
            texture: tex,
            uv: UvRect::FULL,
            tint: Tint::IDENTITY,
            positions: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
            ],
            uvs: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
            ],
        }
    }

    #[test]
    fn empty_finish() {
        let mut b = ElementBatcher::new();
        assert!(b.finish().is_empty());
    }

    #[test]
    fn equal_keys_merge_into_one_batch() {
        let mut b = ElementBatcher::new();
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        let out = b.finish();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].triangle_count(), 3);
    }

    #[test]
    fn different_layers_dont_merge() {
        let mut b = ElementBatcher::new();
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(1, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        let out = b.finish();
        assert_eq!(out.len(), 2);
        assert!(out[0].key.layer < out[1].key.layer);
    }

    #[test]
    fn different_textures_dont_merge() {
        let mut b = ElementBatcher::new();
        b.push(e(0, ShaderKind::Sprite, TextureId(1), DrawEffect::NONE));
        b.push(e(0, ShaderKind::Sprite, TextureId(2), DrawEffect::NONE));
        let out = b.finish();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn different_shaders_dont_merge() {
        let mut b = ElementBatcher::new();
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(0, ShaderKind::Multiply, TextureId::FIRST, DrawEffect::NONE));
        let out = b.finish();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn different_effects_dont_merge() {
        let mut b = ElementBatcher::new();
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(
            0,
            ShaderKind::Sprite,
            TextureId::FIRST,
            DrawEffect::TINTED,
        ));
        let out = b.finish();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn out_of_order_input_gets_sorted() {
        let mut b = ElementBatcher::new();
        b.push(e(5, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        b.push(e(5, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        let out = b.finish();
        assert_eq!(out.len(), 2);
        // Both layer-5 elements land in one batch; layer-0 in the other.
        assert_eq!(out[0].key.layer, 0);
        assert_eq!(out[1].key.layer, 5);
        assert_eq!(out[1].triangle_count(), 2);
    }

    #[test]
    fn finish_clears_batcher() {
        let mut b = ElementBatcher::new();
        b.push(e(0, ShaderKind::Sprite, TextureId::FIRST, DrawEffect::NONE));
        let _ = b.finish();
        assert!(b.is_empty());
    }
}
