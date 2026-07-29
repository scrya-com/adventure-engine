# Roadmap

> Phased delivery plan. Each phase produces one or more git commits. See `git log` for the audit trail.

## Legend

- **Phase** = a milestone that produces a runnable / testable artifact
- **Commit** = one or more phases' files staged atomically with a descriptive message
- **Exit criteria** = the test or example that proves the phase landed

## Phase 0 — Foundation

| # | Commit message | Content |
|---|---|---|
| 1 | `chore: init repo` | `git init`, LICENSE, README, .gitignore, AGENTS.md, CLAUDE.md, rust-toolchain.toml |
| 2 | `docs: design doc + architecture + roadmap` | docs/DESIGN.md, docs/ARCHITECTURE.md, docs/ROADMAP.md |
| 3 | `docs: subsystem design (render/script/audio/save)` | docs/RENDERING.md, SCRIPTING.md, AUDIO.md, SAVE.md |
| 4 | `docs: data formats + AVAL integration` | docs/DATA-FORMATS.md, INTEGRATION-AVAL.md |
| 5 | `docs: decision records (ADRs)` | docs/DECISIONS/0001-0007 |
| 6 | `chore: scaffold workspace + 16 crate stubs` | Cargo.toml workspace + empty crates with `#![deny(missing_docs)]` |
| 7 | `feat(locomotion): fork scene-engine` | copy scene-engine/src/* → crates/locomotion/src/ |
| 8 | `feat(graph): vendor aval_graph` | copy aval_graph/src/lib.rs → crates/graph/src/ |

**Exit:** `cargo build --workspace` passes; `cargo test -p locomotion -p graph` all green.

## Phase 1 — Core + scene data model

| Crate | What lands |
|---|---|
| `core` | `AssetId` newtype, `Error` enum, `glam` re-exports, `tracing` init, `Interner` |
| `scene` | `Scene`, `Room`, `Hotspot`, `Prop`, `Spawn` + RON format |
| `state` | `Tag`, `Tags`, `VarTable`, `StateMachine` |

**Data format:** RON scene files (see `docs/DATA-FORMATS.md`).

**Exit:** `cargo test -p scene -p state` round-trips RON fixtures.

## Phase 2 — Rendering

`render2d` crate, ~1500 LOC:
- `Renderer2D` trait
- `DrawElement`, `ElementBatcher`
- 4 WGSL shaders (sprite, multiply, overlay, post)
- `DrawEffect` bitflags
- wgpu init via winit
- `TextureAtlas` via `guillotiere`

**Examples:** `01-window`, `02-sprite`.

**Exit:** tinted alpha-blended sprite displays.

## Phase 3 — Input + interaction

`input` crate:
- `InputEvent`, `InputState`, `Interactive` trait
- `pick(scene, mouse_pos) -> Vec<EntityId>`
- `Cursor` resource

**Example:** `03-hotspot`.

**Exit:** hover changes cursor; click logs to console.

## Phase 4 — Locomotion wired up (integration proof)

This is the integration test for the forked locomotion crate.

`engine` crate starts here:
- `bevy_ecs::World` with components: `Transform2D`, `Sprite`, `Hotspot`, `Walker`
- Main loop: poll input → world tick → render → audio → vsync
- Click-to-walk system using `locomotion` crate's `WalkGraph::find_path` + `retarget_ardy_walk_along` + `PlantController::tick`

**Example:** `04-walking`.

**Exit:** character walks to clicked point. SE→NW moonwalk regression test still passes.

## Phase 5 — Dialog + UI ✅

`dialogue` crate:
- `DialogTree` (RON) — `entry` + `nodes` map
- `DialogRunner` state machine (`start` / `advance` / `choose`)
- Choice + node `condition` (Rhai); `on_enter` / choice `side_effects`
- Fail-closed choose when condition is false

`scripting` crate:
- `ScriptHost`: `has_tag` / `has_any_tag` / `add_tag` / `set_int`… + var scope
- Side-effect scripts can branch on tags and read vars

`ui` crate:
- Immediate-mode dialog box (panel + choice hit boxes)
- Text glyphs deferred (speaker/line logged + returned as strings)

**Example:** `examples/05-dialog` (workspace member).  
**Fixture:** `assets/dialogs/bob_intro.dialog.ron`.

**Exit:** branching dialog with Rhai conditions — **met**.

## Phase 6 — Audio + save ✅

`audio` crate:
- 4 buses (Master/Music/Sfx/Voice), crossfade, VO + subtitle events
- [`NullMixer`] (headless / CI) + [`KiraMixer`] (kira; `device` feature for cpal)
- Synthetic PCM helper for demos without asset files

`save` crate:
- Versioned header (`SAVG` magic + schema + custom versions)
- MessagePack body + SHA-1 footer, optional PNG thumb, `.meta.json` sidecar
- Migration hook (JSON intermediate) for future custom-version bumps

**Example:** `examples/06-audio-save` (workspace member).

**Exit:** music crossfade on room change; save + restart restores state — **met**.

## Phase 7 — Inventory + verbs ✅

`inventory` crate:
- `Item` + `ItemVerb` (RON) — id, display name, description, tags, verbs
- `Inventory` bag — add/remove/has/count, optional capacity, stackable slots
- `CombineTable` — use item A on B → result / message; **fail closed**
- `ItemCatalog` — definition lookup for combine grants
- `VerbKind` — Look / Use / Talk / Pickup / Give / UseOn
- `VerbCoin` + `InventoryBar` — radial / bar hit-test helpers (immediate-mode UI data)

**Example:** `examples/07-inventory` (workspace member, headless).  
**Fixtures:** `assets/items/*.item.ron`, `assets/items/combine_table.ron`.

**Exit:** pick up, look, combine oil+lamp, fail-closed unknown pair, verb coin hit-test — **met**.

## Phase 8 — Cutscenes + localization + vertical slice

`cutscene` crate — timeline/sequencer with tracks.
`localization` crate — fluent-rs wrapper.

**Example:** `vertical-slice` — one playable room with all subsystems.

**Exit:** playable 1-room demo.

## Phase 9 — Editor tooling

`tools/packer` — `ship.pak` builder (LZ4 + TOC + sha1).
`tools/inspector` — RON editor with `notify`-based hot reload.

## Scope notes

- "Full engine" = phases 0–9. Realistic elapsed effort is months.
- Phased structure lets you ship a vertical slice at phase 8 and continue.
- Bevy_ecs is used **without** the rest of bevy — `[dependencies] bevy_ecs = "0.14"`, not the umbrella crate.
- Rhai is the only scripting surface. No Lua, no Blueprint VM, no custom bytecode.
- AVAL's `.avl` decode is **deferred**. `crates/graph` (the route planning math) is vendored now; `crates/avl_decode` gets added when the engine needs rendered character motion. See `docs/INTEGRATION-AVAL.md`.
- `PresidentialDilema-FastApi` stays separate. If HTTP ever needed, lives there. AVAL is never touched.
