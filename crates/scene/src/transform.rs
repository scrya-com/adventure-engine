//! Transforms, rects, facings.

use adventure_core::math::Vec2;
use serde::{Deserialize, Serialize};

/// 2D transform: position + rotation + scale.
///
/// Coordinates are in normalized background space `[0.0, 1.0]`, y-down
/// (matches `crates/locomotion/src/scene_point.rs::ScenePoint`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform2D {
    /// Position in normalized background space.
    pub pos: Vec2,
    /// Rotation in radians (rarely used in 2D point-and-click).
    #[serde(default)]
    pub rot: f32,
    /// Non-uniform scale. `(1.0, 1.0)` is unscaled.
    #[serde(default = "default_scale")]
    pub scale: Vec2,
}

fn default_scale() -> Vec2 {
    Vec2::ONE
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            rot: 0.0,
            scale: Vec2::ONE,
        }
    }
}

/// An axis-aligned rectangle in normalized background space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Top-left corner.
    pub min: Vec2,
    /// Bottom-right corner.
    pub max: Vec2,
}

impl Rect {
    /// Construct from min + size.
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self {
            min,
            max: min + size,
        }
    }

    /// Width.
    pub fn w(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// Height.
    pub fn h(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Whether a point is inside (closed).
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

/// Cardinal + intercardinal facing.
///
/// Matches the 8-direction walk ring in `crates/locomotion/src/compass.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Facing {
    /// North (away from camera).
    North,
    /// North-east.
    NorthEast,
    /// East (right).
    East,
    /// South-east.
    SouthEast,
    /// South (toward camera).
    South,
    /// South-west.
    SouthWest,
    /// West (left).
    West,
    /// North-west.
    NorthWest,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains() {
        let r = Rect::from_min_size(Vec2::new(0.4, 0.6), Vec2::new(0.1, 0.2));
        assert!(r.contains(Vec2::new(0.45, 0.7)));
        assert!(!r.contains(Vec2::new(0.3, 0.7)));
    }

    #[test]
    fn rect_dimensions() {
        let r = Rect::from_min_size(Vec2::new(0.0, 0.0), Vec2::new(0.5, 0.5));
        assert_eq!(r.w(), 0.5);
        assert_eq!(r.h(), 0.5);
    }

    #[test]
    fn transform_default_is_identity() {
        let t = Transform2D::default();
        assert_eq!(t.pos, Vec2::ZERO);
        assert_eq!(t.scale, Vec2::ONE);
        assert_eq!(t.rot, 0.0);
    }

    #[test]
    fn transform_serde_roundtrip() {
        let t = Transform2D {
            pos: Vec2::new(0.5, 0.5),
            rot: 0.0,
            scale: Vec2::ONE,
        };
        let s = ron::to_string(&t).unwrap();
        let back: Transform2D = ron::from_str(&s).unwrap();
        assert_eq!(back, t);
    }
}
