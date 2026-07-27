//! `Cursor` — what the OS mouse pointer looks like over a region.
//!
//! Mirrors UE's `FSlateApplication::OnCursorQuery`. Hotspots publish
//! their desired cursor; the dispatcher picks the topmost hotspot under
//! the mouse and queries it.

use smol_str::SmolStr;

/// Stable id for a named cursor in the cursor atlas.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CursorId(pub SmolStr);

impl CursorId {
    /// The OS default arrow.
    pub const DEFAULT: CursorId = CursorId(SmolStr::new_inline("default"));
    /// The pointing hand (over an interactive element).
    pub const POINTER: CursorId = CursorId(SmolStr::new_inline("pointer"));
    /// Magnifier / look glass.
    pub const LOOK: CursorId = CursorId(SmolStr::new_inline("look"));
    /// Open hand (drag target).
    pub const GRAB: CursorId = CursorId(SmolStr::new_inline("grab"));
    /// Walking footprints (over a walkable area).
    pub const WALK: CursorId = CursorId(SmolStr::new_inline("walk"));
    /// Talk bubble.
    pub const TALK: CursorId = CursorId(SmolStr::new_inline("talk"));
    /// Use / gear.
    pub const USE: CursorId = CursorId(SmolStr::new_inline("use"));
}

impl Default for CursorId {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// A registered cursor: id + hotspot offset (in pixels, relative to
/// the sprite's top-left) where clicks actually land.
#[derive(Debug, Clone, PartialEq)]
pub struct Cursor {
    /// External id (matches [`CursorId`]).
    pub id: CursorId,
    /// Sprite atlas id (resolved by the renderer).
    pub sprite: adventure_core::AssetId,
    /// Click-point offset within the sprite.
    pub hotspot: adventure_core::math::Vec2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_default_constant() {
        assert_eq!(CursorId::default(), CursorId::DEFAULT);
    }

    #[test]
    fn named_cursors_distinct() {
        let all = [
            CursorId::DEFAULT,
            CursorId::POINTER,
            CursorId::LOOK,
            CursorId::GRAB,
            CursorId::WALK,
            CursorId::TALK,
            CursorId::USE,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert_ne!(a, b, "{a:?} and {b:?} collide");
            }
        }
    }

    #[test]
    fn cursor_carries_hotspot() {
        let c = Cursor {
            id: CursorId::WALK,
            sprite: adventure_core::AssetId::from_path("cursors/walk"),
            hotspot: adventure_core::math::Vec2::new(8.0, 8.0),
        };
        assert_eq!(c.hotspot, adventure_core::math::Vec2::new(8.0, 8.0));
    }
}
