# AVAL Integration

> How the engine integrates with AVAL — a video/animation format + runtime for short prerendered motion. See `aval/flutter/rust/` for the source code; this doc covers our consumption plan.

## What AVAL is

Per `aval/README.md`:

> *"A web format and runtime for short prerendered motion with continuous loops, named application states, authored triggers, bounded transitions, reversals, and packed transparency."*

AVAL ships `.avl` codec bundles (one per codec: AV1, VP9, H.265, H.264) consumed by a player. Think "MP4 + state machine + transitions + codec bundle" — purpose-built for UI motion (hover loops, idle loops, success states).

**Not** an LLM platform. **Not** an HTTP backend. **Not** a game engine. A pure format + runtime.

## What we use now

### `crates/graph/` (vendored aval_graph)

**Pure Rust port** of AVAL's `packages/graph/src/ring-plan.ts` + host BFS hop lists.

- File: `crates/graph/src/lib.rs` (634 LOC)
- License: MIT OR Apache-2.0 (attribution preserved in crate docs)
- Deps: zero

API surface:

```rust
pub enum TieBreak { Forward, Backward }

pub struct RingDefinition {
    pub id: String,
    pub states: Vec<String>,
    pub cyclic: bool,
    pub tie_break: TieBreak,
    pub max_chained_steps: usize,
}

pub struct RingArc {
    pub direction: TieBreak,
    pub states: Vec<String>,  // ordered landings, last is the target
}

pub fn plan_ring_arc(ring: &RingDefinition, from: &str, to: &str) -> Option<RingArc>;
pub fn plan_state_hops(/* direct edge adjacency */ ) -> /* multi-hop route */;
```

Used by `crates/locomotion/` (specifically `compass.rs` and `plant.rs`) to compute walk-state transitions on the 8-direction walk ring.

## What we defer

### Full `.avl` decode (phase 10+, post-vertical-slice)

When the engine needs rendered character motion (not just sprite-tinted animation), we add:

```
crates/avl_decode/   ← vendored from aval/flutter/rust/aval_decode
```

That crate provides:
- H.264 Constrained-Baseline decode via `openh264` (C library, BSD-2-Clause)
- VideoToolbox backend (Apple platforms) in `src/vt/`
- I420 → RGBA conversion
- Frame-credit ledger (frame-accurate scheduling)
- C ABI for FFI (originally for `dart:ffi`; we use it as a normal Rust crate)

**Why defer:** Sprite-based animation covers the vertical slice. Adding `openh264` is a non-trivial dep (C++ build, ~5MB binary). When art is ready to ship as `.avl` bundles, we vendor this crate.

### Authoring motion states

Author character motion as `.avl` bundles:

```
assets/motion/ardy/
├── idle.s1.avl               # idle loop, loop count infinite
├── walk_n.s1.avl             # walk north, single direction
├── walk_s1.avl               # walk south
├── walk_e1.avl               # walk east
├── walk_w1.avl               # walk west
├── walk_ne1.avl              # walk north-east
├── walk_nw1.avl              # walk north-west
├── walk_se1.avl              # walk south-east
├── walk_sw1.avl              # walk south-west
├── turn_n_to_e.s1.avl        # turn transition
├── talk_bob.s1.avl           # talk to Bob (specific NPC)
├── interact_door.s1.avl      # interact with door
└── pickup_key.s1.avl         # pickup animation
```

The `.s1.avl` suffix is AVAL's "single-state" format. Each file has its own state graph and codec bundle. The engine resolves states across files via `aval_graph`'s multi-hop planning.

### Reference example

`aval/flutter/examples/grass_rabbit/` shows how `aval_graph` + `aval_decode` combine. Our integration mirrors that pattern in Rust instead of Dart.

## Integration architecture

```
┌────────────────────────────────────────────────────────────────┐
│ crates/engine/                                                  │
│                                                                  │
│  on click destination:                                           │
│   1. locomotion::WalkGraph::find_path(spawn, dest)              │
│   2. locomotion::retarget_ardy_walk_along(cycle, waypoints)     │
│   3. locomotion::PlantController::tick(dt)                      │
│   4. graph::plan_ring_arc(walk_ring, cur_dir, next_dir)         │
│       ↓                                                          │
│       returns ordered list of states to traverse                │
│       e.g. [walk_n, turn_n_to_e, walk_e]                        │
│   5. avl_decode plays each state's bundle                       │
│   6. render2d paints decoded RGBA frames as textured quads      │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

## Coordination with aval_graph

`aval_graph` is the source of truth for **state-graph math**. We do NOT reimplement its ring-arc or hop-list logic in `crates/locomotion/`. If `compass.rs` or `plant.rs` need to compute "what state sequence gets me from A to B on the walk ring", they call into `crates/graph/`.

```rust
// crates/locomotion/src/compass.rs (existing scene-engine code)
use crate::graph::{RingDefinition, plan_ring_arc};

pub fn resolve_aval_walk_from_screen_heading(/* ... */) -> AvalWalkResolve {
    let ring = RingDefinition { /* 8 walk states */ };
    match plan_ring_arc(&ring, current_state, target_state) {
        Some(arc) => /* traverse arc.states in order */,
        None => /* already at target */,
    }
}
```

The existing scene-engine already references AVAL walk-state names (`walk_southeast`, etc.) as strings. The integration point is already there — we just need to ensure `crates/graph/` is wired in.

## What we DON'T do with AVAL

- ❌ Don't touch the TypeScript packages (`packages/{graph,format,compiler,player-web,element,certification}/`). The action is in `flutter/rust/`.
- ❌ Don't add HTTP integration. AVAL has no server; we never will.
- ❌ Don't modify `aval_graph`'s source after vendoring. If we need a feature, fork into our own crate `crates/graph_extended/` and document the divergence in an ADR.
- ❌ Don't author `.avl` files in-engine. Use AVAL's `avl` CLI compiler (`aval/packages/compiler`) or a separate authoring tool. The engine only consumes.
- ❌ Don't ship `.avl` codec bundles with all 4 codecs unless needed. Pick one codec per target platform (H.264 for compatibility, AV1 for size on modern platforms).

## Heritage / attribution

| Source | Destination | License | Notes |
|---|---|---|---|
| `aval/flutter/rust/aval_graph/` | `crates/graph/` | MIT OR Apache-2.0 | Vendored verbatim; attribution preserved in `crates/graph/Cargo.toml` description and crate docs |
| `aval/flutter/rust/aval_decode/` (future) | `crates/avl_decode/` (future) | BSD-2-Clause | Will vendor when motion rendering is needed |

`aval_graph` was authored by the AVAL team (`@pixel-point`). We are **consumers**, not maintainers — bugs in the ring math should be reported upstream, not patched downstream unless urgent.
