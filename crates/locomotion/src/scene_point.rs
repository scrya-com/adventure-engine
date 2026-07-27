//! Normalized floor plant: feet (x, y) + depth — mirrors Dart `ScenePoint`.

use serde::{Deserialize, Serialize};

/// A point in normalized (0..1) background coordinates.
///
/// `(x, y)` is where the character's feet stand; `depth` (0 = near .. 1 = far)
/// drives perspective scale in the UI (engine stores it for parity).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenePoint {
    /// Horizontal plant in source-image norm space.
    pub x: f64,
    /// Vertical plant in source-image norm space (y-down screen).
    pub y: f64,
    /// Depth 0 near .. 1 far.
    pub depth: f64,
}

impl ScenePoint {
    /// Construct a plant.
    pub const fn new(x: f64, y: f64, depth: f64) -> Self {
        Self { x, y, depth }
    }

    /// Linear interpolation (Dart `ScenePoint.lerp`).
    pub fn lerp(a: Self, b: Self, t: f64) -> Self {
        Self {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            depth: a.depth + (b.depth - a.depth) * t,
        }
    }

    /// Euclidean distance in the xy floor plane (depth ignored).
    pub fn distance_xy(self, other: Self) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn lerp_midpoint() {
        let a = ScenePoint::new(0.0, 0.0, 0.0);
        let b = ScenePoint::new(1.0, 1.0, 1.0);
        let m = ScenePoint::lerp(a, b, 0.5);
        assert_abs_diff_eq!(m.x, 0.5, epsilon = 1e-12);
        assert_abs_diff_eq!(m.y, 0.5, epsilon = 1e-12);
        assert_abs_diff_eq!(m.depth, 0.5, epsilon = 1e-12);
    }
}
