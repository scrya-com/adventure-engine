//! Pure ring arc + multi-hop route planning.
//!
//! Ports:
//! - `packages/graph/src/ring-plan.ts` — `plan_ring_arc` / `resolve_ring_route`
//! - Host BFS hop lists (avl-viewer `pathAlongEdges` / Flutter scene multi-hop) —
//!   `plan_state_hops` over directed edge adjacency, preferring pure walk-ring
//!   paths when both ends are `walk_*` (skip idle as intermediate).
//!
//! Graph install / tick reducer remains host-side (TS `engine.ts` / Dart) for now;
//! this crate locks ring geometry and hop-list math for Rust hosts (WASM / native).

use std::collections::{HashMap, HashSet, VecDeque};

/// Which arc a ring prefers when both directions are equally long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TieBreak {
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingDefinition {
    pub id: String,
    pub states: Vec<String>,
    pub cyclic: bool,
    pub tie_break: TieBreak,
    pub max_chained_steps: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingArc {
    pub direction: TieBreak,
    /// Ordered landings; last entry is the requested target.
    pub states: Vec<String>,
}

/// Choose the shorter arc between two members of one ring.
pub fn plan_ring_arc(ring: &RingDefinition, from: &str, to: &str) -> Option<RingArc> {
    let length = ring.states.len();
    let from_index = ring.states.iter().position(|s| s == from)?;
    let to_index = ring.states.iter().position(|s| s == to)?;
    if from_index == to_index {
        return None;
    }

    let (forward, backward) = if ring.cyclic {
        (
            (to_index + length - from_index) % length,
            (from_index + length - to_index) % length,
        )
    } else {
        let f = if to_index > from_index {
            to_index - from_index
        } else {
            usize::MAX
        };
        let b = if from_index > to_index {
            from_index - to_index
        } else {
            usize::MAX
        };
        (f, b)
    };
    if forward == usize::MAX && backward == usize::MAX {
        return None;
    }

    let direction = if forward < backward {
        TieBreak::Forward
    } else if backward < forward {
        TieBreak::Backward
    } else {
        ring.tie_break
    };
    let distance = if direction == TieBreak::Forward {
        forward
    } else {
        backward
    };
    let offset: isize = if direction == TieBreak::Forward { 1 } else { -1 };
    let mut states = Vec::with_capacity(distance);
    for step in 1..=distance {
        let index = ((from_index as isize + step as isize * offset).rem_euclid(length as isize))
            as usize;
        states.push(ring.states[index].clone());
    }
    Some(RingArc { direction, states })
}

/// Direct neighbour edge map: from → (to → edge_id).
pub type DirectEdges = HashMap<String, HashMap<String, String>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RingRoute {
    None,
    TooLong {
        ring_id: String,
        distance: usize,
    },
    Arc {
        ring_id: String,
        direction: TieBreak,
        states: Vec<String>,
        /// Edge ids along the arc, one per landing.
        step_edge_ids: Vec<String>,
    },
}

/// Resolve authored step edges that walk `from` → `to` along the first capable ring.
pub fn resolve_ring_route(
    rings_by_state: &HashMap<String, Vec<RingDefinition>>,
    direct_edges: &DirectEdges,
    from: &str,
    to: &str,
) -> RingRoute {
    let mut refused: Option<(String, usize)> = None;
    let Some(rings) = rings_by_state.get(from) else {
        return RingRoute::None;
    };
    for ring in rings {
        let Some(arc) = plan_ring_arc(ring, from, to) else {
            continue;
        };
        if arc.states.len() > ring.max_chained_steps {
            refused.get_or_insert_with(|| (ring.id.clone(), arc.states.len()));
            continue;
        }
        let mut step_edge_ids = Vec::with_capacity(arc.states.len());
        let mut cursor = from.to_string();
        let mut ok = true;
        for state in &arc.states {
            match direct_edges.get(&cursor).and_then(|m| m.get(state)) {
                Some(edge_id) => {
                    step_edge_ids.push(edge_id.clone());
                    cursor = state.clone();
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        return RingRoute::Arc {
            ring_id: ring.id.clone(),
            direction: arc.direction,
            states: arc.states,
            step_edge_ids,
        };
    }
    match refused {
        Some((ring_id, distance)) => RingRoute::TooLong { ring_id, distance },
        None => RingRoute::None,
    }
}

/// Directed edge used by multi-hop BFS (`plan_state_hops`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectedEdge {
    pub from: String,
    pub to: String,
}

/// Policy for host-style multi-hop over raw edge adjacency.
///
/// Matches avl-viewer `pathAlongEdges` / Flutter scene: when both ends are
/// `walk_*`, drop edges that touch `idle` so pure walk-ring paths win over
/// equal-length walk→idle→walk cuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HopPolicy {
    /// When `from` and `to` both start with `walk_`, skip edges whose either
    /// endpoint is `idle`.
    pub avoid_idle_intermediate: bool,
}

impl Default for HopPolicy {
    fn default() -> Self {
        Self {
            avoid_idle_intermediate: true,
        }
    }
}

/// Shortest hop list from `from` → `to` over directed edges (BFS).
///
/// - Same state → `Some([])` (exclude `from`; empty means already there).
/// - Reachable → `Some(hops)` ordered landings including `to`, excluding `from`.
/// - Unreachable → `None`.
///
/// With default [`HopPolicy`], walk→walk never routes through `idle` when a
/// pure walk-ring path exists (idle edges are filtered before BFS).
pub fn plan_state_hops(
    from: &str,
    to: &str,
    edges: &[DirectedEdge],
    policy: HopPolicy,
) -> Option<Vec<String>> {
    if from == to {
        return Some(Vec::new());
    }

    let walk_ring = from.starts_with("walk_") && to.starts_with("walk_");
    let skip_idle = policy.avoid_idle_intermediate && walk_ring;

    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for e in edges {
        if skip_idle && (e.from == "idle" || e.to == "idle") {
            continue;
        }
        adj.entry(e.from.as_str()).or_default().push(e.to.as_str());
    }

    let mut queue: VecDeque<(&str, Vec<String>)> = VecDeque::new();
    let mut seen: HashSet<&str> = HashSet::new();
    queue.push_back((from, Vec::new()));
    seen.insert(from);

    while let Some((cur, path)) = queue.pop_front() {
        let Some(neighbors) = adj.get(cur) else {
            continue;
        };
        for &nxt in neighbors {
            if seen.contains(nxt) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(nxt.to_string());
            if nxt == to {
                return Some(next_path);
            }
            seen.insert(nxt);
            queue.push_back((nxt, next_path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Short aliases used by TS ring-turns fixtures.
    fn facing_ring_short(tie_break: TieBreak, cyclic: bool, max_chained: usize) -> RingDefinition {
        RingDefinition {
            id: "facing.walk".into(),
            states: vec![
                "walk_n".into(),
                "walk_ne".into(),
                "walk_e".into(),
                "walk_se".into(),
                "walk_s".into(),
                "walk_sw".into(),
                "walk_w".into(),
                "walk_nw".into(),
            ],
            cyclic,
            tie_break,
            max_chained_steps: max_chained,
        }
    }

    /// Full Scrya-pack names: walk_north … walk_northwest.
    fn facing_ring_full(tie_break: TieBreak, cyclic: bool, max_chained: usize) -> RingDefinition {
        RingDefinition {
            id: "facing.walk".into(),
            states: WALK_FULL.iter().map(|s| (*s).to_string()).collect(),
            cyclic,
            tie_break,
            max_chained_steps: max_chained,
        }
    }

    const WALK_FULL: &[&str] = &[
        "walk_north",
        "walk_northeast",
        "walk_east",
        "walk_southeast",
        "walk_south",
        "walk_southwest",
        "walk_west",
        "walk_northwest",
    ];

    fn step_id(from: &str, to: &str) -> String {
        format!("facing.walk.{from}.{to}")
    }

    /// Bidirectional adjacent edges for a cyclic facing ring (edge list order
    /// matches TS `facingGraph`: for each index, forward then reverse).
    fn ring_edges(states: &[&str]) -> (DirectEdges, Vec<DirectedEdge>) {
        let n = states.len();
        let mut direct: DirectEdges = HashMap::new();
        let mut edges = Vec::new();
        for i in 0..n {
            let from = states[i];
            let to = states[(i + 1) % n];
            let fwd = step_id(from, to);
            let bak = step_id(to, from);
            direct
                .entry(from.to_string())
                .or_default()
                .insert(to.to_string(), fwd.clone());
            direct
                .entry(to.to_string())
                .or_default()
                .insert(from.to_string(), bak.clone());
            edges.push(DirectedEdge {
                from: from.to_string(),
                to: to.to_string(),
            });
            edges.push(DirectedEdge {
                from: to.to_string(),
                to: from.to_string(),
            });
        }
        (direct, edges)
    }

    fn rings_index(ring: RingDefinition) -> HashMap<String, Vec<RingDefinition>> {
        let mut map: HashMap<String, Vec<RingDefinition>> = HashMap::new();
        for s in &ring.states {
            map.entry(s.clone()).or_default().push(ring.clone());
        }
        map
    }

    #[test]
    fn shorter_arc_and_landings() {
        let ring = facing_ring_short(TieBreak::Forward, true, 4);
        let arc = plan_ring_arc(&ring, "walk_n", "walk_e").unwrap();
        assert_eq!(arc.direction, TieBreak::Forward);
        assert_eq!(arc.states, vec!["walk_ne", "walk_e"]);
        let arc = plan_ring_arc(&ring, "walk_n", "walk_w").unwrap();
        assert_eq!(arc.direction, TieBreak::Backward);
        assert_eq!(arc.states, vec!["walk_nw", "walk_w"]);
    }

    #[test]
    fn half_turn_tie_break() {
        let forward = facing_ring_short(TieBreak::Forward, true, 4);
        let backward = facing_ring_short(TieBreak::Backward, true, 4);
        assert_eq!(
            plan_ring_arc(&forward, "walk_n", "walk_s")
                .unwrap()
                .direction,
            TieBreak::Forward
        );
        assert_eq!(
            plan_ring_arc(&backward, "walk_n", "walk_s")
                .unwrap()
                .direction,
            TieBreak::Backward
        );
    }

    #[test]
    fn non_cyclic_no_wrap() {
        let line = facing_ring_short(TieBreak::Forward, false, 16);
        assert!(plan_ring_arc(&line, "walk_n", "walk_n").is_none());
        assert!(plan_ring_arc(&line, "walk_n", "sit").is_none());
        let arc = plan_ring_arc(&line, "walk_nw", "walk_ne").unwrap();
        assert_eq!(
            arc.states,
            vec![
                "walk_w", "walk_sw", "walk_s", "walk_se", "walk_e", "walk_ne"
            ]
        );
    }

    // ── Full-name 8-dir goldens (Scrya packs) ──────────────────────────────

    #[test]
    fn full_walk_se_to_nw_four_hops_either_arc() {
        let ring = facing_ring_full(TieBreak::Forward, true, 4);
        let arc = plan_ring_arc(&ring, "walk_southeast", "walk_northwest").unwrap();
        // Equal half-turn: tieBreak forward → SE→S→SW→W→NW (4 hops).
        assert_eq!(arc.states.len(), 4);
        assert_eq!(arc.states.last().unwrap(), "walk_northwest");
        assert_eq!(
            arc.states,
            vec![
                "walk_south",
                "walk_southwest",
                "walk_west",
                "walk_northwest",
            ]
        );

        // Backward tie-break yields the other arc of equal length.
        let ring_b = facing_ring_full(TieBreak::Backward, true, 4);
        let arc_b = plan_ring_arc(&ring_b, "walk_southeast", "walk_northwest").unwrap();
        assert_eq!(arc_b.states.len(), 4);
        assert_eq!(
            arc_b.states,
            vec![
                "walk_east",
                "walk_northeast",
                "walk_north",
                "walk_northwest",
            ]
        );
    }

    #[test]
    fn full_walk_east_to_north_two_hops_via_northeast() {
        let ring = facing_ring_full(TieBreak::Forward, true, 4);
        let arc = plan_ring_arc(&ring, "walk_east", "walk_north").unwrap();
        assert_eq!(arc.direction, TieBreak::Backward);
        assert_eq!(arc.states, vec!["walk_northeast", "walk_north"]);
    }

    #[test]
    fn full_walk_same_state_empty_path() {
        let ring = facing_ring_full(TieBreak::Forward, true, 4);
        assert!(plan_ring_arc(&ring, "walk_east", "walk_east").is_none());

        let (_, edges) = ring_edges(WALK_FULL);
        let hops = plan_state_hops(
            "walk_east",
            "walk_east",
            &edges,
            HopPolicy::default(),
        )
        .unwrap();
        assert!(hops.is_empty());
    }

    // ── resolve_ring_route: max_chained_steps TooLong vs Arc ────────────────

    #[test]
    fn resolve_ring_route_arc_within_max_chained() {
        let ring = facing_ring_full(TieBreak::Forward, true, 4);
        let (direct, _) = ring_edges(WALK_FULL);
        let index = rings_index(ring);
        let route = resolve_ring_route(
            &index,
            &direct,
            "walk_east",
            "walk_north",
        );
        match route {
            RingRoute::Arc {
                states,
                step_edge_ids,
                direction,
                ..
            } => {
                assert_eq!(direction, TieBreak::Backward);
                assert_eq!(states, vec!["walk_northeast", "walk_north"]);
                assert_eq!(step_edge_ids.len(), 2);
            }
            other => panic!("expected Arc, got {other:?}"),
        }
    }

    #[test]
    fn resolve_ring_route_too_long_when_over_max_chained() {
        // Half-turn is 4 steps; max 3 → TooLong (reachable-but-refused).
        let ring = facing_ring_full(TieBreak::Forward, true, 3);
        let (direct, _) = ring_edges(WALK_FULL);
        let index = rings_index(ring);
        let route = resolve_ring_route(
            &index,
            &direct,
            "walk_southeast",
            "walk_northwest",
        );
        match route {
            RingRoute::TooLong { ring_id, distance } => {
                assert_eq!(ring_id, "facing.walk");
                assert_eq!(distance, 4);
            }
            other => panic!("expected TooLong, got {other:?}"),
        }
    }

    #[test]
    fn resolve_ring_route_half_turn_at_max_is_arc() {
        // distance == max_chained_steps is allowed (not >).
        let ring = facing_ring_full(TieBreak::Forward, true, 4);
        let (direct, _) = ring_edges(WALK_FULL);
        let index = rings_index(ring);
        let route = resolve_ring_route(
            &index,
            &direct,
            "walk_southeast",
            "walk_northwest",
        );
        match route {
            RingRoute::Arc { states, .. } => {
                assert_eq!(states.len(), 4);
                assert_eq!(states.last().unwrap(), "walk_northwest");
            }
            other => panic!("expected Arc at max, got {other:?}"),
        }
    }

    #[test]
    fn resolve_ring_route_none_when_no_ring() {
        let ring = facing_ring_full(TieBreak::Forward, true, 4);
        let (direct, _) = ring_edges(WALK_FULL);
        let index = rings_index(ring);
        let route = resolve_ring_route(&index, &direct, "idle", "walk_north");
        assert_eq!(route, RingRoute::None);
    }

    // ── plan_state_hops (BFS multi-hop) ─────────────────────────────────────

    #[test]
    fn bfs_se_to_nw_four_hops() {
        let (_, edges) = ring_edges(WALK_FULL);
        let hops = plan_state_hops(
            "walk_southeast",
            "walk_northwest",
            &edges,
            HopPolicy::default(),
        )
        .expect("reachable");
        assert_eq!(hops.len(), 4);
        assert_eq!(hops.last().unwrap(), "walk_northwest");
        // Either equal-length arc is valid; check membership of intermediate set.
        let set: HashSet<&str> = hops.iter().map(|s| s.as_str()).collect();
        assert!(set.contains("walk_northwest"));
        // Must stay on the walk ring (no idle).
        assert!(!set.iter().any(|s| *s == "idle"));
    }

    #[test]
    fn bfs_east_to_north_via_northeast() {
        let (_, edges) = ring_edges(WALK_FULL);
        let hops = plan_state_hops(
            "walk_east",
            "walk_north",
            &edges,
            HopPolicy::default(),
        )
        .expect("reachable");
        assert_eq!(hops, vec!["walk_northeast", "walk_north"]);
    }

    #[test]
    fn bfs_same_state_empty() {
        let (_, edges) = ring_edges(WALK_FULL);
        assert_eq!(
            plan_state_hops("walk_south", "walk_south", &edges, HopPolicy::default()),
            Some(vec![])
        );
    }

    #[test]
    fn bfs_unreachable_is_none() {
        let edges = vec![DirectedEdge {
            from: "walk_east".into(),
            to: "walk_northeast".into(),
        }];
        assert_eq!(
            plan_state_hops(
                "walk_east",
                "walk_west",
                &edges,
                HopPolicy::default()
            ),
            None
        );
    }

    #[test]
    fn bfs_walk_to_walk_prefers_ring_over_idle() {
        // Unweighted BFS without the policy would treat E→idle→N as equal to
        // E→NE→N and may never play turn bridges.
        let mut edges = ring_edges(WALK_FULL).1;
        edges.push(DirectedEdge {
            from: "walk_east".into(),
            to: "idle".into(),
        });
        edges.push(DirectedEdge {
            from: "idle".into(),
            to: "walk_north".into(),
        });
        // Also add idle first in adj order to bias naive BFS.
        edges.insert(
            0,
            DirectedEdge {
                from: "walk_east".into(),
                to: "idle".into(),
            },
        );

        let hops = plan_state_hops(
            "walk_east",
            "walk_north",
            &edges,
            HopPolicy {
                avoid_idle_intermediate: true,
            },
        )
        .expect("reachable via ring");
        assert_eq!(hops, vec!["walk_northeast", "walk_north"]);
        assert!(!hops.iter().any(|s| s == "idle"));

        // With policy off, idle shortcut of length 2 competes; BFS may pick it
        // depending on edge order (idle edges are first).
        let hops_naive = plan_state_hops(
            "walk_east",
            "walk_north",
            &edges,
            HopPolicy {
                avoid_idle_intermediate: false,
            },
        )
        .expect("reachable");
        assert_eq!(hops_naive.len(), 2);
        // First edge from walk_east in list is idle → idle path wins.
        assert_eq!(hops_naive, vec!["idle", "walk_north"]);
    }

    #[test]
    fn bfs_non_walk_may_use_idle() {
        let edges = vec![
            DirectedEdge {
                from: "sit".into(),
                to: "idle".into(),
            },
            DirectedEdge {
                from: "idle".into(),
                to: "wave".into(),
            },
        ];
        let hops = plan_state_hops("sit", "wave", &edges, HopPolicy::default()).unwrap();
        assert_eq!(hops, vec!["idle", "wave"]);
    }
}
