//! `pick()` — find hotspots under a point, ordered by layer.
//!
//! Mirrors UE's `FSlateApplication::FindWindowUnderCursor` collapsed
//! to a polygon hit-test. We walk the room's hotspot list, run a
//! point-in-polygon check (the same algorithm `Room::hotspot_at` uses
//! but made reusable here), and return matches ordered topmost-first
//! (i.e. higher `layer` first; ties broken by authoring order —
//! later hotspots win).

use adventure_core::math::Vec2;
use adventure_scene::hotspot::Hotspot;

/// Result of a single pick — the hotspot id + its layer (for ordering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickHit {
    /// The hotspot's id (matches [`Hotspot::id`]).
    pub id: smol_str::SmolStr,
    /// Authoring layer of the hotspot.
    pub layer: i32,
}

/// Find all hotspots under `pos`, ordered topmost-first.
///
/// For most use cases you want [`pick_topmost`] which returns just the
/// front-most hit.
pub fn pick(hotspots: &[Hotspot], pos: Vec2) -> Vec<PickHit> {
    let mut hits: Vec<PickHit> = hotspots
        .iter()
        .enumerate()
        .filter(|(_, h)| point_in_polygon(pos, &h.polygon))
        .map(|(i, h)| PickHit {
            id: h.id.clone(),
            // Hotspots don't have an explicit layer field yet; we use the
            // authoring index as a tiebreaker (later = on top).
            layer: i as i32,
        })
        .collect();
    // Sort topmost-first: higher layer first; ties keep authoring order
    // (Rust's `sort_by` is stable, so we don't break ties explicitly).
    hits.sort_by(|a, b| b.layer.cmp(&a.layer));
    hits
}

/// Just the topmost hit, if any.
pub fn pick_topmost(hotspots: &[Hotspot], pos: Vec2) -> Option<PickHit> {
    pick(hotspots, pos).into_iter().next()
}

/// Standard ray-cast point-in-polygon test.
///
/// Mirrors `Hotspot::contains` in adventure-scene — kept here as a
/// reusable helper for any caller that needs to test against an
/// arbitrary polygon (not just a `Hotspot`).
pub fn point_in_polygon(p: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x
                < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y + f32::EPSILON) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;
    use adventure_scene::hotspot::{Cursor, Hotspot, HotspotKind, OnClick};

    fn square_hotspot(id: &str, min: (f32, f32), max: (f32, f32)) -> Hotspot {
        Hotspot {
            id: smol_str::SmolStr::new_inline(id),
            kind: HotspotKind::Examine,
            polygon: vec![
                Vec2::new(min.0, min.1),
                Vec2::new(max.0, min.1),
                Vec2::new(max.0, max.1),
                Vec2::new(min.0, max.1),
            ],
            cursor: Cursor::Default,
            on_click: OnClick::Action(smol_str::SmolStr::new_inline("noop")),
        }
    }

    #[test]
    fn pick_empty_returns_empty() {
        let hits = pick(&[], Vec2::new(0.5, 0.5));
        assert!(hits.is_empty());
    }

    #[test]
    fn pick_hits_inside() {
        let h = square_hotspot("a", (0.0, 0.0), (1.0, 1.0));
        let hits = pick(&[h], Vec2::new(0.5, 0.5));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "a");
    }

    #[test]
    fn pick_misses_outside() {
        let h = square_hotspot("a", (0.0, 0.0), (0.5, 0.5));
        assert!(pick(&[h], Vec2::new(0.9, 0.9)).is_empty());
    }

    #[test]
    fn pick_topmost_orders_by_authoring_index() {
        // Without an explicit layer, the later-authored hotspot wins ties.
        let hotspots = vec![
            square_hotspot("first", (0.0, 0.0), (1.0, 1.0)),
            square_hotspot("second", (0.0, 0.0), (1.0, 1.0)),
        ];
        let top = pick_topmost(&hotspots, Vec2::new(0.5, 0.5)).unwrap();
        assert_eq!(top.id, "second");
    }

    #[test]
    fn point_in_polygon_basic_square() {
        let sq = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        assert!(point_in_polygon(Vec2::new(0.5, 0.5), &sq));
        assert!(!point_in_polygon(Vec2::new(1.5, 0.5), &sq));
    }

    #[test]
    fn point_in_polygon_degenerate() {
        assert!(!point_in_polygon(Vec2::ZERO, &[]));
        assert!(!point_in_polygon(Vec2::ZERO, &[Vec2::ZERO]));
        assert!(!point_in_polygon(
            Vec2::ZERO,
            &[Vec2::ZERO, Vec2::new(1.0, 1.0)]
        ));
    }

    #[test]
    fn point_in_polygon_concave() {
        // L-shape (concave).
        let l = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 0.5),
            Vec2::new(0.5, 0.5),
            Vec2::new(0.5, 1.0),
            Vec2::new(0.0, 1.0),
        ];
        // Inside the main body.
        assert!(point_in_polygon(Vec2::new(0.25, 0.25), &l));
        // In the concave notch — outside the L.
        assert!(!point_in_polygon(Vec2::new(0.75, 0.75), &l));
    }
}
