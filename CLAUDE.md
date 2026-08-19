# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

This is a Rust workspace for **Ariadne** (product name; repo/crate paths still say `adventure-engine`), a native point-and-click adventure game engine — wgpu + bevy_ecs (world only) + Rhai + RON. See `AGENTS.md` for the agent-oriented layout summary **and the CodeGraphContext workflow** (callers/callees before repo-wide grep).

## Commands

```bash
cargo build --workspace                 # build everything
cargo test --workspace                  # run all tests
cargo test -p adventure-locomotion      # test one crate (crate names are adventure-*, dirs are crates/*)
cargo test -p adventure-locomotion plant::tests::moonwalk_regression_hold_until_route_settled  # single test
cargo run -p example-08-shawshank-pac   # run an example (see examples/ for the numbered set)
cargo doc --workspace --no-deps --open
cargo clippy --workspace
```

There is no CI config and no root rustfmt/clippy.toml — `rust-toolchain.toml` pins `stable` with `rustfmt` + `clippy` components.

Workspace crate names are `adventure-<dir>` (e.g. `crates/locomotion` → `adventure-locomotion`), except `crates/workflow_graph` → `adventure-workflow-graph`. Examples are `example-NN-<name>` matching their `examples/` directory.

## Architecture

16 library crates + `workflow_graph` (17), layered strictly bottom-up (see `docs/ARCHITECTURE.md` for the full diagram) — a crate only depends on layers below it:

1. **Foundation** (no internal deps): `core` (AssetId, Error, glam re-exports, string interning), `graph` (vendored `aval_graph`, ring-arc route planning, zero deps)
2. **Locomotion**: `locomotion` (forked `scene-engine` — walk graphs, plant FSM, retargeting), depends only on `core` + `graph`
3. **Domain types**: `scene`, `state`, `inventory`, `dialogue` (+ `scripting`), `save`, `assets`, `localization`
4. **I/O**: `render2d` (wgpu + Slate-style batcher), `audio` (kira 4-bus), `input` (winit → `InputEvent`), `ui`, `cutscene`
5. **Engine**: `engine` — owns the `bevy_ecs` `World`, defines `Resource`s (e.g. `SceneGraph`, `PendingClick`), schedules systems (`click_to_walk_system`, `walker_tick_system`) via `FrameSchedule`/`run_frame`. There is no `App` struct — engine state lives as ECS resources on `World`, and callers drive the frame loop themselves.
6. **Tools**: `examples/*` (one vertical slice per phase), `tools/` (packer, RON inspector — phase 9, not yet built)

`crates/workflow_graph` is a separate, mostly-standalone concern (see below) — it doesn't sit in the engine dependency chain.

Per-frame data flow (winit → input::dispatch → engine systems → render2d/audio → present) and the asset resolution / save-load flows are diagrammed in full in `docs/ARCHITECTURE.md` — read it before touching frame ordering or the asset cache.

### Data & scripting

- **RON** for all authored data (scenes, dialog trees, items, manifests) — hot-reloadable, human-edited. Saves are versioned serde (see `docs/SAVE.md`, `docs/DATA-FORMATS.md`).
- **Rhai** is the only scripting language (dialog conditions, side effects, inventory combine rules) — sandboxed, no GC. See `docs/SCRIPTING.md`.
- Refs: `Arc<T>` (hard), `AssetId` (soft, path-only), `Weak<T>` (LRU-evictable).

### workflow_graph (crates/workflow_graph, examples/09)

A separate tool that *statically parses* (not executes) Grok Build `.rhai` workflow files into a `WorkflowGraph` struct, emitting Mermaid or JSON, or serving both plus a small HTTP API over that data for a sibling Flutter UI (`../PresidentialDilema-FastApi/workflow_graph_ui/`). Rust parses; Flutter renders. Entry point: `parse_workflow_file()`; HTTP routes documented in `examples/09-workflow-graph/README.md`.

### Forked / vendored code — do not casually modify

- `crates/locomotion/` is **forked** from `scene-engine` (`~/Documents/GitHub/PresidentialDilema-FastApi/scene-engine`, MIT). It encodes non-obvious invariants — most importantly the SE→NW moonwalk regression (`crates/locomotion/src/plant.rs`, test `moonwalk_regression_hold_until_route_settled`) and gait/ring-arc locks. Don't change locomotion/route math without confirming these tests still pass.
- `crates/graph/` is **vendored** from `aval_graph` (`~/Documents/GitHub/aval/flutter/rust/aval_graph`, MIT OR Apache-2.0). Keep it byte-equivalent to upstream unless explicitly deciding to diverge (write an ADR).

## Conventions

- `#![deny(missing_docs)]` on every library crate — every `pub` item needs a doc comment.
- Tests are inline `#[cfg(test)] mod tests` per file, not a top-level `tests/` dir (matches the forked scene-engine pattern).
- UE 5.8 reference paths in comments use the form `UE: SlateCore/Rendering/ElementBatcher.h:245` — the design deliberately borrows UE 5.8 abstraction *patterns* (Slate batcher, gameplay tags, streaming assets, versioned saves), not code; full mapping in `docs/DESIGN.md`.
- No `unsafe` outside `crates/render2d/` (wgpu FFI only).
- Never hand-edit `Cargo.lock` — let cargo manage it.
- Each roadmap phase (`docs/ROADMAP.md`) maps to one or more commits; phases 0–7 are done, 8 (cutscenes/i18n) and 9 (editor tooling) are pending.

## When making changes

- **New feature**: check `docs/ROADMAP.md` for its phase, read the relevant subsystem doc (`docs/RENDERING.md`, `docs/SCRIPTING.md`, `docs/AUDIO.md`, `docs/SAVE.md`), and if it touches the foundation (data format, crate boundary, scripting story) write an ADR in `docs/DECISIONS/` first (see `0001`–`0007` for the format/precedent).
- **Bug fix**: reproduce with a test first; find root cause rather than papering over with `unwrap()`/`panic!()`; keep the fix small and targeted.
- Don't add new crates without an ADR, don't add core dependencies without an ADR.
- Explicitly out of scope: Lua/Python/custom bytecode VM, an HTTP server for the engine itself (workflow_graph's HTTP API is the one sanctioned exception, for the Flutter sidecar), the umbrella `bevy` crate (only `bevy_ecs` is used).
