//! Verb coin — radial UI data + hit-test helpers.
//!
//! The coin is pure layout math (no GPU). Hosts rebuild it each frame
//! from the active target's available verbs (immediate-mode, per DESIGN).

use adventure_core::math::Vec2;

use crate::verb::VerbKind;

/// One sector of the verb coin (for drawing).
#[derive(Clone, Debug, PartialEq)]
pub struct VerbSector {
    /// Verb in this sector.
    pub verb: VerbKind,
    /// Sector index (0..n).
    pub index: usize,
    /// Start angle in radians (0 = east, CCW; sectors start from top = -PI/2).
    pub start_angle: f32,
    /// End angle in radians.
    pub end_angle: f32,
    /// Mid-angle for label placement.
    pub mid_angle: f32,
    /// Suggested label anchor in screen space.
    pub label_pos: Vec2,
}

/// Radial verb selector centered on a click / hotspot.
///
/// Layout: equal wedges, clockwise from the top, between `inner_radius`
/// and `outer_radius`. Clicks inside the dead-zone (inner circle) or
/// outside the outer radius miss.
#[derive(Clone, Debug, PartialEq)]
pub struct VerbCoin {
    /// Screen-space center.
    pub center: Vec2,
    /// Outer radius in pixels.
    pub outer_radius: f32,
    /// Inner dead-zone radius in pixels.
    pub inner_radius: f32,
    /// Verbs in sector order (clockwise from top).
    pub verbs: Vec<VerbKind>,
}

impl VerbCoin {
    /// Default outer / inner radii (px).
    pub const DEFAULT_OUTER: f32 = 72.0;
    /// Dead-zone radius.
    pub const DEFAULT_INNER: f32 = 20.0;

    /// Build a coin at `center` with the given verbs.
    pub fn new(center: Vec2, verbs: impl Into<Vec<VerbKind>>) -> Self {
        Self {
            center,
            outer_radius: Self::DEFAULT_OUTER,
            inner_radius: Self::DEFAULT_INNER,
            verbs: verbs.into(),
        }
    }

    /// Standard 4-verb coin (Look / Use / Talk / Pickup).
    pub fn standard(center: Vec2) -> Self {
        Self::new(center, VerbKind::COIN_DEFAULT.to_vec())
    }

    /// 5-verb coin including Give.
    pub fn with_give(center: Vec2) -> Self {
        Self::new(center, VerbKind::COIN_WITH_GIVE.to_vec())
    }

    /// Override radii (builder).
    pub fn with_radii(mut self, outer: f32, inner: f32) -> Self {
        self.outer_radius = outer.max(1.0);
        self.inner_radius = inner.clamp(0.0, self.outer_radius - 1.0);
        self
    }

    /// Number of sectors.
    pub fn sector_count(&self) -> usize {
        self.verbs.len()
    }

    /// Sector descriptors for rendering.
    pub fn sectors(&self) -> Vec<VerbSector> {
        let n = self.verbs.len();
        if n == 0 {
            return Vec::new();
        }
        let step = std::f32::consts::TAU / n as f32;
        // Start at top (-PI/2).
        let origin = -std::f32::consts::FRAC_PI_2;
        let label_r = (self.inner_radius + self.outer_radius) * 0.5;
        self.verbs
            .iter()
            .enumerate()
            .map(|(i, &verb)| {
                let start = origin + step * i as f32;
                let end = start + step;
                let mid = start + step * 0.5;
                let label_pos = Vec2::new(
                    self.center.x + mid.cos() * label_r,
                    self.center.y + mid.sin() * label_r,
                );
                VerbSector {
                    verb,
                    index: i,
                    start_angle: start,
                    end_angle: end,
                    mid_angle: mid,
                    label_pos,
                }
            })
            .collect()
    }

    /// Hit-test a screen position. Returns the verb under the pointer.
    ///
    /// Misses (None) when:
    /// - no verbs
    /// - distance from center < inner_radius or > outer_radius
    pub fn hit_test(&self, pos: Vec2) -> Option<VerbKind> {
        let n = self.verbs.len();
        if n == 0 {
            return None;
        }
        let dx = pos.x - self.center.x;
        let dy = pos.y - self.center.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < self.inner_radius || dist > self.outer_radius {
            return None;
        }
        // atan2: east=0, CCW. Remap so top sector is index 0.
        let angle = dy.atan2(dx); // [-PI, PI]
        let origin = -std::f32::consts::FRAC_PI_2;
        let mut rel = angle - origin;
        // Normalize to [0, TAU)
        while rel < 0.0 {
            rel += std::f32::consts::TAU;
        }
        while rel >= std::f32::consts::TAU {
            rel -= std::f32::consts::TAU;
        }
        let step = std::f32::consts::TAU / n as f32;
        let idx = (rel / step).floor() as usize;
        self.verbs.get(idx.min(n - 1)).copied()
    }

    /// Axis-aligned bounds that fully contain the coin (for dirty rects).
    pub fn bounds(&self) -> (Vec2, Vec2) {
        let r = self.outer_radius;
        (
            Vec2::new(self.center.x - r, self.center.y - r),
            Vec2::new(self.center.x + r, self.center.y + r),
        )
    }

    /// Whether `pos` is inside the outer circle (including dead zone).
    pub fn contains(&self, pos: Vec2) -> bool {
        let dx = pos.x - self.center.x;
        let dy = pos.y - self.center.y;
        dx * dx + dy * dy <= self.outer_radius * self.outer_radius
    }
}

/// Inventory bar hit-test: horizontal row of equal cells.
#[derive(Clone, Debug, PartialEq)]
pub struct InventoryBar {
    /// Top-left of the bar.
    pub origin: Vec2,
    /// Cell size (square).
    pub cell: f32,
    /// Gap between cells.
    pub gap: f32,
    /// Number of visible slots (including empty).
    pub slot_count: usize,
}

impl InventoryBar {
    /// Build a bar.
    pub fn new(origin: Vec2, cell: f32, gap: f32, slot_count: usize) -> Self {
        Self {
            origin,
            cell,
            gap,
            slot_count,
        }
    }

    /// Hit-test → slot index, or None.
    pub fn hit_test(&self, pos: Vec2) -> Option<usize> {
        if self.slot_count == 0 || self.cell <= 0.0 {
            return None;
        }
        let local_x = pos.x - self.origin.x;
        let local_y = pos.y - self.origin.y;
        if local_y < 0.0 || local_y > self.cell {
            return None;
        }
        if local_x < 0.0 {
            return None;
        }
        let stride = self.cell + self.gap;
        let idx = (local_x / stride).floor() as usize;
        if idx >= self.slot_count {
            return None;
        }
        // Must be inside the cell, not the gap
        let cell_x = local_x - idx as f32 * stride;
        if cell_x > self.cell {
            return None;
        }
        Some(idx)
    }

    /// Top-left of slot `i`.
    pub fn slot_origin(&self, i: usize) -> Vec2 {
        Vec2::new(
            self.origin.x + i as f32 * (self.cell + self.gap),
            self.origin.y,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_top_sector_is_look() {
        let coin = VerbCoin::standard(Vec2::new(100.0, 100.0));
        // Point just above center (top = Look)
        let p = Vec2::new(100.0, 100.0 - 40.0);
        assert_eq!(coin.hit_test(p), Some(VerbKind::Look));
    }

    #[test]
    fn dead_zone_misses() {
        let coin = VerbCoin::standard(Vec2::new(0.0, 0.0));
        assert_eq!(coin.hit_test(Vec2::ZERO), None);
        assert_eq!(coin.hit_test(Vec2::new(5.0, 0.0)), None);
    }

    #[test]
    fn outside_misses() {
        let coin = VerbCoin::standard(Vec2::new(0.0, 0.0));
        assert_eq!(coin.hit_test(Vec2::new(200.0, 0.0)), None);
    }

    #[test]
    fn four_sectors() {
        let coin = VerbCoin::standard(Vec2::ZERO);
        assert_eq!(coin.sectors().len(), 4);
        assert_eq!(coin.sectors()[0].verb, VerbKind::Look);
        assert_eq!(coin.sectors()[1].verb, VerbKind::Use);
        assert_eq!(coin.sectors()[2].verb, VerbKind::Talk);
        assert_eq!(coin.sectors()[3].verb, VerbKind::Pickup);
    }

    #[test]
    fn inventory_bar_hit() {
        let bar = InventoryBar::new(Vec2::new(10.0, 200.0), 32.0, 4.0, 4);
        // First cell
        assert_eq!(bar.hit_test(Vec2::new(20.0, 210.0)), Some(0));
        // Gap between 0 and 1
        assert_eq!(bar.hit_test(Vec2::new(10.0 + 33.0, 210.0)), None);
        // Second cell starts at 10+32+4=46
        assert_eq!(bar.hit_test(Vec2::new(50.0, 210.0)), Some(1));
        // Above bar
        assert_eq!(bar.hit_test(Vec2::new(20.0, 100.0)), None);
    }
}
