//! Layout primitives — pixel-space rectangles, alignment, padding.

use adventure_core::math::Vec2;

/// Axis-aligned rectangle in pixel space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Top-left corner.
    pub min: Vec2,
    /// Size (width, height).
    pub size: Vec2,
}

impl Rect {
    /// Construct from top-left + size.
    pub fn new(min: Vec2, size: Vec2) -> Self {
        Self { min, size }
    }

    /// Construct from a centre point + size.
    pub fn from_center(center: Vec2, size: Vec2) -> Self {
        Self {
            min: Vec2::new(center.x - size.x * 0.5, center.y - size.y * 0.5),
            size,
        }
    }

    /// Width.
    pub fn w(&self) -> f32 {
        self.size.x
    }

    /// Height.
    pub fn h(&self) -> f32 {
        self.size.y
    }

    /// Top edge.
    pub fn top(&self) -> f32 {
        self.min.y
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f32 {
        self.min.y + self.size.y
    }

    /// Left edge.
    pub fn left(&self) -> f32 {
        self.min.x
    }

    /// Right edge.
    pub fn right(&self) -> f32 {
        self.min.x + self.size.x
    }

    /// Centre.
    pub fn center(&self) -> Vec2 {
        Vec2::new(
            self.min.x + self.size.x * 0.5,
            self.min.y + self.size.y * 0.5,
        )
    }

    /// Shrink inward by `inset` pixels on every side.
    pub fn shrink(&self, inset: f32) -> Rect {
        Rect {
            min: Vec2::new(self.min.x + inset, self.min.y + inset),
            size: Vec2::new(
                (self.size.x - 2.0 * inset).max(0.0),
                (self.size.y - 2.0 * inset).max(0.0),
            ),
        }
    }

    /// Point-in-rect test (inclusive of edges).
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.left()
            && p.x <= self.right()
            && p.y >= self.top()
            && p.y <= self.bottom()
    }
}

/// Anchor within a parent rect (used to position children).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Top-left of parent.
    TopLeft,
    /// Top-centre of parent.
    TopCenter,
    /// Bottom-centre of parent.
    BottomCenter,
    /// Centre of parent.
    Center,
}

/// Position a `child_size` rect inside `parent` per `anchor`, with
/// the supplied margin from the parent edge.
pub fn place(parent: Rect, child_size: Vec2, anchor: Anchor, margin: f32) -> Rect {
    let c = match anchor {
        Anchor::TopLeft => Vec2::new(
            parent.left() + margin + child_size.x * 0.5,
            parent.top() + margin + child_size.y * 0.5,
        ),
        Anchor::TopCenter => Vec2::new(parent.center().x, parent.top() + margin + child_size.y * 0.5),
        Anchor::BottomCenter => Vec2::new(
            parent.center().x,
            parent.bottom() - margin - child_size.y * 0.5,
        ),
        Anchor::Center => parent.center(),
    };
    Rect::from_center(c, child_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: Vec2, b: Vec2) -> bool {
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4
    }

    #[test]
    fn rect_edges() {
        let r = Rect::new(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        assert_eq!(r.left(), 10.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.top(), 20.0);
        assert_eq!(r.bottom(), 70.0);
        assert!(approx_eq(r.center(), Vec2::new(60.0, 45.0)));
    }

    #[test]
    fn contains_inclusive() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        assert!(r.contains(Vec2::new(0.0, 0.0)));
        assert!(r.contains(Vec2::new(10.0, 10.0)));
        assert!(r.contains(Vec2::new(5.0, 5.0)));
        assert!(!r.contains(Vec2::new(11.0, 5.0)));
        assert!(!r.contains(Vec2::new(5.0, -1.0)));
    }

    #[test]
    fn shrink_inward() {
        let r = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
        let s = r.shrink(10.0);
        assert_eq!(s.left(), 10.0);
        assert_eq!(s.right(), 90.0);
        assert_eq!(s.size, Vec2::new(80.0, 80.0));
    }

    #[test]
    fn shrink_clamps_to_zero() {
        let r = Rect::new(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let s = r.shrink(100.0);
        assert_eq!(s.size, Vec2::ZERO);
    }

    #[test]
    fn place_top_left() {
        let parent = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        let child = place(parent, Vec2::new(100.0, 50.0), Anchor::TopLeft, 10.0);
        assert_eq!(child.left(), 10.0);
        assert_eq!(child.top(), 10.0);
    }

    #[test]
    fn place_bottom_center() {
        let parent = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(800.0, 600.0));
        let child = place(parent, Vec2::new(400.0, 100.0), Anchor::BottomCenter, 20.0);
        assert!(approx_eq(child.center(), Vec2::new(400.0, 530.0)));
    }
}
