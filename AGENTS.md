# adventure-engine — Agent Guide

A native Rust point-and-click adventure engine. Inspired by Unreal Engine 5.8 architecture (see `docs/DESIGN.md`), built around forked locomotion code from `scene-engine` and vendored route-planning from `aval_graph`.

## Working in this repo

- **Build:** `cargo build --workspace`
- **Test:** `cargo test --workspace`
- **Run examples:** `cargo run --example <name>` (see `examples/` dir for current set)
- **Docs:** `cargo doc --workspace --no-deps --open`

## Layout

```
crates/
  core/         # glam, AssetId, Error, tracing
  locomotion/   # FORKED from scene-engine — walk graphs, plant FSM
  graph/        # VENDORED aval_graph — ring-arc route planning
  scene/        # Scene/Room/Hotspot/Prop + RON format
  state/        # GameplayTags + VarTable + StateTree-inspired FSM
  dialogue/     # dialog trees + Rhai conditions
  inventory/    # items + verb coin
  save/         # versioned serde save format
  assets/       # AssetRecord manifest + async loader + Arc/Weak
  render2d/     # wgpu + Slate-style ElementBatcher + 4 shaders
  audio/        # kira wrapper (Master/Music/Sfx/Voice)
  input/        # winit events → Interactive dispatch
  ui/           # immediate-mode menus/dialog + retained HUD
  scripting/    # Rhai wrapper
  cutscene/     # timeline/sequencer
  localization/ # fluent-rs
  engine/       # main loop, owns World + systems
docs/           # design docs + ADRs
examples/       # numbered examples per phase
tools/          # packer, inspector (phase 9)
```

## Conventions

- `#![deny(missing_docs)]` is on every crate. Document every public item.
- Tests live inline as `#[cfg(test)] mod tests` per file, not in a separate `tests/` dir (matches scene-engine pattern).
- Data is RON (readable) in dev, packed (LZ4 + TOC) for shipping.
- All session/script work is Rhai (sandboxed, no GC) — no Lua, no Blueprint VM.
- Refs are `Arc<T>` (hard) / `AssetId` (soft, path-only) / `Weak<T>` (LRU-evictable).
- UE 5.8 reference paths in comments use the form `UE: SlateCore/Rendering/ElementBatcher.h:245`.
- Each phase = one or more commits. See `docs/ROADMAP.md`.

## Heritage

- `crates/locomotion/` — forked from `scene-engine` at `~/Documents/GitHub/PresidentialDilema-FastApi/scene-engine` (MIT, John D. Pope). Re-license kept as MIT.
- `crates/graph/` — vendored from `aval_graph` at `~/Documents/GitHub/aval/flutter/rust/aval_graph` (MIT OR Apache-2.0). Attribution preserved in crate docs.

Do **not** modify either of these crates' locomotion/graph math without checking the upstream tests still pass — they encode non-obvious invariants (SE→NW moonwalk, gait locks, ring arcs).

## Decisions

Architectural decisions are recorded in `docs/DECISIONS/0001-*.md` onward. Read them before proposing changes to the foundation.
