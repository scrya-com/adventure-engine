# adventure-engine

A small, native Rust point-and-click adventure game engine.

Inspired by an architectural analysis of Unreal Engine 5.8 — keeping the load-bearing abstractions (slate-style element batcher, gameplay tags, streaming assets, versioned saves) while dropping everything irrelevant to 2D point-and-click (3D rendering, physics, networking, Blueprint VM, mark-sweep GC, cooking).

## Status

| Phase | Status | What lands |
|---|---|---|
| 0 — Foundation | in progress | repo, design docs, forked locomotion SDK, vendored graph math |
| 1 — Core + scene data model | pending | RON scene format, gameplay tags, var table |
| 2 — Rendering | pending | wgpu + Slate-style batcher, 4 shaders |
| 3 — Input + interaction | pending | click/hover dispatch, hotspot picking |
| 4 — Locomotion wired up | pending | click-to-walk using forked scene-engine code |
| 5 — Dialog + UI | pending | RON dialog trees + Rhai conditions |
| 6 — Audio + save | pending | kira 4-bus mixer, versioned save files |
| 7 — Inventory + verbs | pending | verb coin UI, item combining |
| 8 — Cutscenes + i18n | pending | timeline/sequencer, fluent-rs |
| 9 — Editor tooling | pending | asset packer, RON inspector |

See `docs/ROADMAP.md` for the full plan.

## Architecture

16-crate workspace under `crates/`. See `docs/ARCHITECTURE.md` for the dependency graph.

**Foundation crates:**
- `core` — glam re-exports, AssetId, Error, tracing
- `locomotion` — forked from scene-engine (walk graphs, plant FSM, retargeting)
- `graph` — vendored aval_graph (ring-arc route planning)

**Engine crates:**
- `scene`, `state`, `dialogue`, `inventory`, `save`, `assets`
- `render2d`, `audio`, `input`, `ui`
- `scripting` (Rhai), `cutscene`, `localization`
- `engine` (main loop)

## Source heritage

| Code | Origin | License |
|---|---|---|
| `crates/locomotion/` | forked from `scene-engine` (PresidentialDilema-FastApi) | MIT |
| `crates/graph/` | vendored from `aval_graph` (aval/flutter/rust) | MIT OR Apache-2.0 |
| All other crates | original | MIT |

The design borrows patterns (not code) from Unreal Engine 5.8 — see `docs/DECISIONS/` for the architectural decisions and `docs/DESIGN.md` for the full UE → Rust mapping.

## Building

```bash
cargo build --workspace
cargo test --workspace
cargo run --example 02-sprite   # phase 2
```

## License

MIT — see [LICENSE](LICENSE).
