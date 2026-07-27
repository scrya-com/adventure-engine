//! Props (placed sprites / decorations).
//!
//! A prop is a visible sprite placed at a transform in a room. Walkers
//! (characters) are also props with extra `Walker` component in ECS —
//! see `crates/engine/`.

use adventure_core::{AssetId, SmolStr};
use serde::{Deserialize, Serialize};

use crate::transform::Transform2D;

/// A sprite reference: which atlas/region to draw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    /// Asset id of the sprite atlas (resolves via `crates/assets`).
    pub atlas: AssetId,
    /// Optional region name within the atlas. None = whole image.
    #[serde(default)]
    pub region: Option<SmolStr>,
    /// Render layer (higher = drawn later / on top).
    #[serde(default = "default_layer")]
    pub layer: i32,
}

fn default_layer() -> i32 {
    0
}

/// A placed visible object in a room.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    /// Stable id within the room (e.g. `"prop_barrel"`).
    pub id: SmolStr,
    /// What to draw.
    pub sprite: Sprite,
    /// Where in the room.
    #[serde(default)]
    pub transform: Transform2D,
}

#[cfg(test)]
mod tests {
    use super::*;
    use adventure_core::math::Vec2;

    #[test]
    fn prop_serde_minimal() {
        let p = Prop {
            id: SmolStr::new("prop_barrel"),
            sprite: Sprite {
                atlas: AssetId::from_path("sprites/barrel"),
                region: None,
                layer: 10,
            },
            transform: Transform2D {
                pos: Vec2::new(0.3, 0.7),
                rot: 0.0,
                scale: Vec2::ONE,
            },
        };
        let s = ron::to_string(&p).unwrap();
        let back: Prop = ron::from_str(&s).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn sprite_default_layer() {
        // AssetId serializes as u64; layer defaults to 0 when omitted.
        let s: Sprite = ron::from_str(r#"(atlas: 1234)"#).unwrap();
        assert_eq!(s.layer, 0);
        assert!(s.region.is_none());
    }
}
