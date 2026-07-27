//! Floor walk graph — mirrors Dart `ardy_walk_graph.dart` (guide paths).

use crate::scene_point::ScenePoint;

fn hypot(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

/// Kind of walk-graph node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkNodeKind {
    /// Free-floor grid / manual anchor.
    Floor,
    /// Approach pad in front of a SAM prop.
    Approach,
}

/// One node on the loft floor (norm bg coords).
#[derive(Clone, Debug)]
pub struct WalkGraphNode {
    /// Stable id.
    pub id: String,
    /// Plant.
    pub point: ScenePoint,
    /// Optional SAM label.
    pub label: Option<String>,
    /// Floor vs approach.
    pub kind: WalkNodeKind,
}

/// Axis-aligned obstacle in norm space (Dart `Rect`).
#[derive(Clone, Copy, Debug)]
pub struct NormRect {
    /// Left.
    pub left: f64,
    /// Top.
    pub top: f64,
    /// Right.
    pub right: f64,
    /// Bottom.
    pub bottom: f64,
}

impl NormRect {
    /// Contains a point (inclusive edges).
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }

    /// Width / height helpers.
    pub fn width(&self) -> f64 {
        self.right - self.left
    }
    /// Height.
    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }
}

/// Minimal element view for graph build (subset of Dart `SceneElement`).
#[derive(Clone, Debug)]
pub struct GraphElement {
    /// SAM label.
    pub label: String,
    /// ltrb box.
    pub box_norm: NormRect,
    /// Baseline y.
    pub baseline_y: f64,
}

/// Undirected walk graph + Dijkstra pathfinding.
#[derive(Clone, Debug)]
pub struct WalkGraph {
    /// Nodes.
    pub nodes: Vec<WalkGraphNode>,
    /// Undirected edges as index pairs.
    pub edges: Vec<(usize, usize)>,
}

impl WalkGraph {
    /// Empty check.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// All node plants (control points).
    pub fn control_points(&self) -> Vec<ScenePoint> {
        self.nodes.iter().map(|n| n.point).collect()
    }

    /// Shortest path `from` → `to` as waypoints (includes endpoints).
    pub fn find_path(&self, from: ScenePoint, to: ScenePoint) -> Vec<ScenePoint> {
        if self.nodes.is_empty() {
            return simplify_walk_path(vec![from, to]);
        }
        let dist = from.distance_xy(to);
        if dist < 0.01 {
            return vec![from, to];
        }

        let start = self.nearest_index(from, false);
        let goal = self.nearest_index(to, true);
        if start == goal {
            let hub = self.nodes[start].point;
            let d_hub = from.distance_xy(hub);
            if d_hub < 0.02 {
                return simplify_walk_path(vec![from, to]);
            }
            return simplify_walk_path(vec![from, hub, to]);
        }

        let Some(prev) = self.dijkstra(start, goal) else {
            return simplify_walk_path(vec![from, to]);
        };

        let mut chain = Vec::new();
        let mut cur = goal;
        loop {
            chain.push(cur);
            if cur == start {
                break;
            }
            match prev[cur] {
                Some(p) => cur = p,
                None => return simplify_walk_path(vec![from, to]),
            }
        }
        chain.reverse();
        let mut path = vec![from];
        for i in chain {
            path.push(self.nodes[i].point);
        }
        path.push(to);
        simplify_walk_path(path)
    }

    fn nearest_index(&self, p: ScenePoint, prefer_approach: bool) -> usize {
        let mut best = 0;
        let mut best_score = f64::INFINITY;
        for (i, n) in self.nodes.iter().enumerate() {
            let d = p.distance_xy(n.point);
            let score = if prefer_approach && n.kind == WalkNodeKind::Approach {
                d * 0.85
            } else {
                d
            };
            if score < best_score {
                best_score = score;
                best = i;
            }
        }
        best
    }

    fn dijkstra(&self, start: usize, goal: usize) -> Option<Vec<Option<usize>>> {
        let n = self.nodes.len();
        let mut adj: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        for &(a, b) in &self.edges {
            if a >= n || b >= n || a == b {
                continue;
            }
            let w = self.nodes[a].point.distance_xy(self.nodes[b].point);
            adj[a].push((b, w));
            adj[b].push((a, w));
        }

        let mut dist = vec![f64::INFINITY; n];
        let mut prev = vec![None; n];
        let mut used = vec![false; n];
        dist[start] = 0.0;

        for _ in 0..n {
            let mut u = None;
            let mut best = f64::INFINITY;
            for i in 0..n {
                if !used[i] && dist[i] < best {
                    best = dist[i];
                    u = Some(i);
                }
            }
            let u = match u {
                Some(u) if best.is_finite() => u,
                _ => break,
            };
            if u == goal {
                return Some(prev);
            }
            used[u] = true;
            for &(v, w) in &adj[u] {
                let nd = dist[u] + w;
                if nd < dist[v] {
                    dist[v] = nd;
                    prev[v] = Some(u);
                }
            }
        }
        if dist[goal].is_finite() {
            Some(prev)
        } else {
            None
        }
    }
}

fn is_non_floor_prop(label: &str) -> bool {
    let l = label.to_lowercase();
    l.contains("window")
        || l.contains("picture")
        || l.contains("frame")
        || l.contains("hanging")
        || l.contains("shelf")
        || l.contains("bookshelf")
        || l.contains("monitor")
        || l.contains("cushion")
}

fn is_blocking_prop(label: &str) -> bool {
    if is_non_floor_prop(label) {
        return false;
    }
    let l = label.to_lowercase();
    l.contains("sofa")
        || l.contains("couch")
        || l.contains("desk")
        || l.contains("table")
        || l.contains("armchair")
        || l.contains("chair")
        || l.contains("rug")
}

/// Build walk graph from SAM-like elements + free-floor anchors.
pub fn build_walk_graph_from_elements(elements: &[GraphElement]) -> WalkGraph {
    let mut nodes: Vec<WalkGraphNode> = Vec::new();
    let mut obstacles: Vec<NormRect> = Vec::new();

    const XS: [f64; 6] = [0.18, 0.32, 0.45, 0.55, 0.68, 0.82];
    const YS: [f64; 4] = [0.58, 0.68, 0.78, 0.88];
    let mut floor_i = 0;
    for &y in &YS {
        for &x in &XS {
            nodes.push(WalkGraphNode {
                id: format!("floor_{floor_i}"),
                point: ScenePoint::new(x, y, (1.0 - y).clamp(0.0, 1.0)),
                label: None,
                kind: WalkNodeKind::Floor,
            });
            floor_i += 1;
        }
    }

    let defaults: [(&str, f64, f64); 5] = [
        ("plant", 0.50, 0.78),
        ("sofa_front", 0.22, 0.78),
        ("front_center", 0.55, 0.88),
        ("dining_front", 0.72, 0.80),
        ("mid_aisle", 0.40, 0.62),
    ];
    for (id, x, y) in defaults {
        nodes.push(WalkGraphNode {
            id: format!("anchor_{id}"),
            point: ScenePoint::new(x, y, (1.0 - y).clamp(0.0, 1.0)),
            label: Some(id.to_string()),
            kind: WalkNodeKind::Floor,
        });
    }

    let mut approach_i = 0;
    for el in elements {
        if el.baseline_y < 0.50 {
            continue;
        }
        if is_non_floor_prop(&el.label) {
            continue;
        }
        let cx = (el.box_norm.left + el.box_norm.right) / 2.0;
        let ay = (el.baseline_y + 0.035).clamp(0.52, 0.95);
        nodes.push(WalkGraphNode {
            id: format!("approach_{approach_i}"),
            point: ScenePoint::new(cx, ay, (1.0 - ay).clamp(0.0, 1.0)),
            label: Some(el.label.clone()),
            kind: WalkNodeKind::Approach,
        });
        approach_i += 1;

        if is_blocking_prop(&el.label) {
            let b = el.box_norm;
            let h = b.height();
            obstacles.push(NormRect {
                left: b.left + b.width() * 0.08,
                top: b.top + h * 0.15,
                right: b.right - b.width() * 0.08,
                bottom: (el.baseline_y - 0.02).clamp(b.top, b.bottom),
            });
        }
    }

    // Dedup near-duplicate nodes.
    let mut unique: Vec<WalkGraphNode> = Vec::new();
    for n in nodes {
        let dup = unique
            .iter()
            .any(|u| u.point.distance_xy(n.point) < 0.035);
        if !dup {
            unique.push(n);
        }
    }

    const MAX_EDGE: f64 = 0.28;
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for i in 0..unique.len() {
        for j in (i + 1)..unique.len() {
            let a = unique[i].point;
            let b = unique[j].point;
            let d = a.distance_xy(b);
            if d < 0.02 || d > MAX_EDGE {
                continue;
            }
            if segment_blocked(a, b, &obstacles) {
                continue;
            }
            edges.push((i, j));
        }
    }

    let mut degree = vec![0usize; unique.len()];
    for &(a, b) in &edges {
        degree[a] += 1;
        degree[b] += 1;
    }
    for i in 0..unique.len() {
        if degree[i] > 0 {
            continue;
        }
        let mut best_j = None;
        let mut best_d = f64::INFINITY;
        for j in 0..unique.len() {
            if i == j {
                continue;
            }
            let d = unique[i].point.distance_xy(unique[j].point);
            if d < best_d && d < 0.45 {
                best_d = d;
                best_j = Some(j);
            }
        }
        if let Some(j) = best_j {
            edges.push((i, j));
            degree[i] += 1;
            degree[j] += 1;
        }
    }

    WalkGraph {
        nodes: unique,
        edges,
    }
}

/// Default loft graph with no SAM elements.
pub fn build_default_loft_walk_graph() -> WalkGraph {
    build_walk_graph_from_elements(&[])
}

fn segment_blocked(a: ScenePoint, b: ScenePoint, obstacles: &[NormRect]) -> bool {
    if obstacles.is_empty() {
        return false;
    }
    const SAMPLES: i32 = 8;
    for s in 1..SAMPLES {
        let t = s as f64 / SAMPLES as f64;
        let x = a.x + (b.x - a.x) * t;
        let y = a.y + (b.y - a.y) * t;
        for o in obstacles {
            if o.contains(x, y) {
                return true;
            }
        }
    }
    false
}

fn dedupe_waypoints(pts: Vec<ScenePoint>) -> Vec<ScenePoint> {
    if pts.is_empty() {
        return pts;
    }
    let mut out = vec![pts[0]];
    for p in pts.into_iter().skip(1) {
        let a = *out.last().unwrap();
        if a.distance_xy(p) > 0.012 {
            out.push(p);
        }
    }
    out
}

/// Remove micro-segments and collinear midpoints (Dart `simplifyWalkPath`).
pub fn simplify_walk_path(pts: Vec<ScenePoint>) -> Vec<ScenePoint> {
    let mut path = dedupe_waypoints(pts);
    if path.len() <= 2 {
        return path;
    }

    let mut changed = true;
    while changed && path.len() > 2 {
        changed = false;
        let mut next = vec![path[0]];
        for i in 1..path.len() - 1 {
            let a = *next.last().unwrap();
            let b = path[i];
            let c = path[i + 1];
            let abx = b.x - a.x;
            let aby = b.y - a.y;
            let bcx = c.x - b.x;
            let bcy = c.y - b.y;
            let lab = hypot(abx, aby);
            let lbc = hypot(bcx, bcy);
            if lab < 1e-9 || lbc < 1e-9 {
                changed = true;
                continue;
            }
            let cos = (abx * bcx + aby * bcy) / (lab * lbc);
            if cos > 0.978 {
                changed = true;
                continue;
            }
            next.push(b);
        }
        next.push(*path.last().unwrap());
        path = next;
    }
    dedupe_waypoints(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_graph_has_nodes_and_edges() {
        let g = build_default_loft_walk_graph();
        assert!(g.nodes.len() > 8);
        assert!(g.edges.len() > 8);
    }

    #[test]
    fn path_left_to_right_has_hubs() {
        let g = build_default_loft_walk_graph();
        let from = ScenePoint::new(0.18, 0.88, 0.12);
        let to = ScenePoint::new(0.82, 0.78, 0.22);
        let path = g.find_path(from, to);
        assert!(path.len() >= 3);
        assert!((path[0].x - from.x).abs() < 1e-9);
        assert!((path.last().unwrap().x - to.x).abs() < 1e-9);
    }

    #[test]
    fn approach_pads_for_furniture() {
        let elements = vec![
            GraphElement {
                label: "leather sofa".into(),
                box_norm: NormRect {
                    left: 0.14,
                    top: 0.52,
                    right: 0.29,
                    bottom: 0.70,
                },
                baseline_y: 0.70,
            },
            GraphElement {
                label: "window".into(),
                box_norm: NormRect {
                    left: 0.12,
                    top: 0.27,
                    right: 0.23,
                    bottom: 0.57,
                },
                baseline_y: 0.57,
            },
        ];
        let g = build_walk_graph_from_elements(&elements);
        let approaches: Vec<_> = g
            .nodes
            .iter()
            .filter(|n| n.kind == WalkNodeKind::Approach)
            .collect();
        assert!(!approaches.is_empty());
        assert!(approaches.iter().any(|n| n.label.as_deref() == Some("leather sofa")));
        assert!(!approaches
            .iter()
            .any(|n| n.label.as_ref().is_some_and(|l| l.contains("window"))));
    }

    #[test]
    fn simplify_drops_collinear() {
        let path = simplify_walk_path(vec![
            ScenePoint::new(0.1, 0.8, 0.2),
            ScenePoint::new(0.3, 0.8, 0.2),
            ScenePoint::new(0.5, 0.8, 0.2),
            ScenePoint::new(0.7, 0.8, 0.2),
        ]);
        assert!(path.len() <= 3);
        assert!((path[0].x - 0.1).abs() < 1e-9);
        assert!((path.last().unwrap().x - 0.7).abs() < 1e-9);
    }
}
