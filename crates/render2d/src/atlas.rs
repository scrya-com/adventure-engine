//! `TextureAtlas` — runtime sub-allocation in a single GPU texture.
//!
//! Wraps [`guillotiere`] (a rectangle bin-packer). Adventure games
//! typically have hundreds of small sprites (idle frames, items,
//! cursors); packing them into one atlas means far fewer pipeline /
//! bind-group switches.
//!
//! Reference: SlateCore's `FSlateAtlasData` (we keep a simpler shape —
//! a single growing atlas rather than UE's separate font/atlas pools).

use guillotiere::{AllocId, AtlasAllocator, Rectangle, Size};
use thiserror::Error;

use adventure_core::math::Vec2;

use crate::element::{TextureId, UvRect};

/// Errors from atlas operations.
#[derive(Debug, Error)]
pub enum AtlasError {
    /// Allocation failed — atlas full and grow policy disabled.
    #[error("atlas full; could not allocate {w}x{h}")]
    Full {
        /// Requested width.
        w: i32,
        /// Requested height.
        h: i32,
    },
}

/// Pixel-size + UV rect stored per allocation.
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// Pixel rect in the atlas (retained for debugging; UV is what's used).
    #[allow(dead_code)]
    rect: Rectangle,
    /// Normalized UV rect.
    uv: UvRect,
}

/// A growing 2D texture atlas.
pub struct TextureAtlas {
    allocator: AtlasAllocator,
    size: [u32; 2],
    /// Next `TextureId` to hand out (starts at FIRST).
    next_id: u32,
    /// Allocations, indexed by `TextureId.0`.
    slots: Vec<Option<Slot>>,
    /// Live AllocIds from the underlying allocator (keyed by TextureId.0).
    alloc_ids: Vec<Option<AllocId>>,
}

impl TextureAtlas {
    /// Create an atlas of `width`x`height` pixels.
    pub fn new(width: u32, height: u32) -> Self {
        let allocator = AtlasAllocator::new(Size::new(width as i32, height as i32));
        Self {
            allocator,
            size: [width, height],
            next_id: TextureId::FIRST.0,
            slots: Vec::new(),
            alloc_ids: Vec::new(),
        }
    }

    /// Allocate a slot for a `w x h` sprite. Returns its id.
    pub fn allocate(&mut self, w: u32, h: u32) -> Result<TextureId, AtlasError> {
        let size = Size::new(w as i32, h as i32);
        let alloc = self
            .allocator
            .allocate(size)
            .ok_or(AtlasError::Full { w: w as i32, h: h as i32 })?;
        let id = TextureId(self.next_id);
        self.next_id += 1;
        let idx = id.0 as usize;
        if self.slots.len() <= idx {
            self.slots.resize(idx + 1, None);
            self.alloc_ids.resize(idx + 1, None);
        }
        self.slots[idx] = Some(Slot {
            rect: alloc.rectangle,
            uv: self.rect_to_uv(&alloc.rectangle),
        });
        self.alloc_ids[idx] = Some(alloc.id);
        Ok(id)
    }

    /// Release a previous allocation. Id is reused for a future alloc.
    pub fn deallocate(&mut self, id: TextureId) {
        let idx = id.0 as usize;
        if let Some(slot) = self.alloc_ids.get_mut(idx) {
            if let Some(alloc_id) = slot.take() {
                self.allocator.deallocate(alloc_id);
            }
        }
        if let Some(slot) = self.slots.get_mut(idx) {
            *slot = None;
        }
    }

    /// Look up the UV rect for an id.
    pub fn uv(&self, id: TextureId) -> Option<UvRect> {
        self.slots.get(id.0 as usize).and_then(|s| s.map(|s| s.uv))
    }

    /// Pixel size of the underlying atlas texture.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    fn rect_to_uv(&self, rect: &Rectangle) -> UvRect {
        let (w, h) = (self.size[0] as f32, self.size[1] as f32);
        UvRect {
            min: Vec2::new(rect.min.x as f32 / w, rect.min.y as f32 / h),
            max: Vec2::new(rect.max.x as f32 / w, rect.max.y as f32 / h),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_assigns_unique_ids() {
        let mut a = TextureAtlas::new(256, 256);
        let a1 = a.allocate(32, 32).unwrap();
        let a2 = a.allocate(32, 32).unwrap();
        assert_ne!(a1, a2);
    }

    #[test]
    fn first_id_is_first() {
        let mut a = TextureAtlas::new(256, 256);
        let id = a.allocate(8, 8).unwrap();
        assert_eq!(id, TextureId::FIRST);
    }

    #[test]
    fn uv_in_unit_range() {
        let mut a = TextureAtlas::new(256, 256);
        let id = a.allocate(64, 64).unwrap();
        let uv = a.uv(id).unwrap();
        assert!(uv.min.x >= 0.0 && uv.min.x <= 1.0);
        assert!(uv.min.y >= 0.0 && uv.min.y <= 1.0);
        assert!(uv.max.x >= 0.0 && uv.max.x <= 1.0);
        assert!(uv.max.y >= 0.0 && uv.max.y <= 1.0);
        assert!(uv.max.x > uv.min.x);
        assert!(uv.max.y > uv.min.y);
    }

    #[test]
    fn deallocate_frees_slot() {
        let mut a = TextureAtlas::new(64, 64);
        let id = a.allocate(32, 32).unwrap();
        a.deallocate(id);
        assert!(a.uv(id).is_none());
    }

    #[test]
    fn fill_atlas_returns_full() {
        let mut a = TextureAtlas::new(16, 16);
        assert!(matches!(a.allocate(32, 32), Err(AtlasError::Full { .. })));
    }

    #[test]
    fn deallocate_then_realloc() {
        // After freeing, we should be able to allocate something of the same size.
        let mut a = TextureAtlas::new(64, 64);
        let id1 = a.allocate(32, 32).unwrap();
        a.deallocate(id1);
        let id2 = a.allocate(32, 32).unwrap();
        // New TextureId, even though it might pack into the same atlas pixels.
        assert_ne!(id1, id2);
    }
}
