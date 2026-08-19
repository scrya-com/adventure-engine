# adventure-engine — Agent Guide

A native Rust point-and-click adventure engine. Inspired by Unreal Engine 5.8 architecture (see `docs/DESIGN.md`), built around forked locomotion code from `scene-engine` and vendored route-planning from `aval_graph`.

## Working in this repo

- **Build:** `cargo build --workspace`
- **Test:** `cargo test --workspace`
- **Run examples:** `cargo run -p example-08-shawshank-pac` (numbered `examples/`)
- **Docs:** `cargo doc --workspace --no-deps --open`

## Code graph first (CodeGraphContext)

This workspace is indexed in a local CodeGraphContext graph (FalkorDB Lite). **Use the graph before grepping the whole tree** for symbols, callers, callees, impact, dead code, or crate coupling.

Both this repo and the sibling VN host `../romp` are in the same graph (`cgc list`).

### When MCP `codegraphcontext` is connected

Discover tools with the MCP search, then call them. Grok names look like `codegraphcontext__<tool>`.

| Question | Tool | Args |
|---|---|---|
| Where is `Foo`? | `find_code` | `query: "Foo"`, optional `repo_path` |
| Who calls `advance`? | `analyze_code_relationships` | `query_type: "find_callers"`, `target: "advance"` |
| What does it call? | `analyze_code_relationships` | `query_type: "find_callees"` |
| Direct + indirect | `analyze_code_relationships` | `query_type: "find_all_callers"` or `find_all_callees`, `depth` 1–20 |
| Path A → B | `analyze_code_relationships` | `query_type: "call_chain"`, `target: "start_fn->end_fn"` |
| Unused fns | `analyze_code_relationships` | `query_type: "dead_code"` |
| Crate imports | `analyze_code_relationships` | `query_type: "module_deps"` |
| Custom graph walk | `execute_cypher_query` | read-only Cypher |
| Stale index? | `list_indexed_repositories` then `add_code_to_graph` | `repo_path` = this root; poll `check_job_status` |

Scope with `repo_path` (`…/adventure-engine` vs `…/romp`) so results do not mix hosts.

Do **not** use the graph for string literals, comments, RON/Rhai contents, or “show me this file”. Those stay `grep` / `read_file`.

### CLI fallback (no MCP)

```bash
cgc list
cgc find name StoryRunner
cgc analyze callers advance
cgc analyze calls advance
cgc analyze chain boot_at advance
cgc analyze dead-code
cgc query 'MATCH (f:Function {name:"advance"}) RETURN f.path LIMIT 10'
cgc index .          # after large structural edits; --force to rebuild
```

`.cgcignore` already skips `target/` and media. After renaming a public symbol or adding a crate, re-index (or `watch_directory`) before trusting caller lists.

Install notes and human setup: `README.md` § Code graph.

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
  scenario/     # VN Story/Stmt/StoryRunner (RON → IR)
  movie/        # ffmpeg-pipe VP9 → wgpu frames
  workflow_graph/ # parse Grok .rhai workflows (not the game loop)
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
