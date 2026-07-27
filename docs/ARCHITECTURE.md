# Architecture — Crate Dependency Graph

> How the 16 crates compose. Read alongside `docs/DESIGN.md`.

## Dependency layers

```
┌──────────────────────────────────────────────────────────────────────────┐
│ LAYER 1 — Foundation (no internal deps)                                  │
│                                                                          │
│   core                ← glam, tracing, string_interner, AssetId, Error   │
│   graph      (vendored aval_graph — pure Rust, zero deps)                │
└──────────────────────────────────────────────────────────────────────────┘
            ▲                              ▲
            │                              │
┌──────────────────────────────────────────────────────────────────────────┐
│ LAYER 2 — Locomotion (depends on Layer 1 only)                           │
│                                                                          │
│   locomotion   ← FORK of scene-engine; uses core + graph                 │
└──────────────────────────────────────────────────────────────────────────┘
            ▲
            │
┌──────────────────────────────────────────────────────────────────────────┐
│ LAYER 3 — Domain types (depend on Layer 1, optionally 2)                 │
│                                                                          │
│   scene         ← Scene, Room, Hotspot, Prop + RON format                │
│   state         ← GameplayTags, VarTable, StateMachine                   │
│   inventory     ← Item, slots, combine rules                             │
│   dialogue      ← DialogTree, Rhai conditions (depends on scripting)     │
│   scripting     ← Rhai wrapper                                           │
│   save          ← versioned serde header + migrations                    │
│   assets        ← AssetRecord, async loader, Arc/Weak                    │
│   localization  ← fluent-rs wrapper                                      │
└──────────────────────────────────────────────────────────────────────────┘
            ▲                              ▲
            │                              │
┌──────────────────────────────────────────────────────────────────────────┐
│ LAYER 4 — I/O (depend on Layer 1 + 3)                                    │
│                                                                          │
│   render2d   ← wgpu + Slate-style batcher; consumes assets               │
│   audio      ← kira 4-bus; consumes assets                               │
│   input      ← winit events; produces InputEvent stream                  │
│   ui         ← immediate-mode + retained HUD; uses render2d              │
│   cutscene   ← timeline/sequencer; drives render2d + audio + state       │
└──────────────────────────────────────────────────────────────────────────┘
            ▲
            │
┌──────────────────────────────────────────────────────────────────────────┐
│ LAYER 5 — Engine (depends on everything)                                 │
│                                                                          │
│   engine   ← bevy_ecs World + systems + main loop + plugin registry      │
└──────────────────────────────────────────────────────────────────────────┘
            ▲
            │
┌──────────────────────────────────────────────────────────────────────────┐
│ LAYER 6 — Tools (depend on engine or specific crates)                    │
│                                                                          │
│   examples/*    ← vertical slices per phase                              │
│   tools/packer  ← asset packing (FPakEntry-style)                        │
│   tools/inspector ← RON editor with hot reload                           │
└──────────────────────────────────────────────────────────────────────────┘
```

## Data flow per frame

```
                ┌─────────────────────────────────────┐
                │ winit event queue                   │
                └─────────────┬───────────────────────┘
                              │ WindowEvent / DeviceEvent
                              ▼
                ┌─────────────────────────────────────┐
                │ input::dispatch                     │
                │ - translate to InputEvent enum      │
                │ - update InputState (polled)        │
                │ - maintain capture / focus          │
                └─────────────┬───────────────────────┘
                              │ buffered InputEvents
                              ▼
   ┌──────────────────────────────────────────────────────────────────┐
   │ engine::App::tick(dt)                                            │
   │                                                                  │
   │  1. PreUpdate  system:    drain input events                     │
   │  2. GameUpdate system:    scene systems (dialog, verbs, scripts) │
   │  3. Locomotion system:    advance PlantController, retarget walk │
   │  4. Walker system:        position sprites                       │
   │  5. PostUpdate system:    camera follow, scene transition        │
   │  6. Cleanup system:       drop expired handles                   │
   └─────────────┬────────────────────────────────────┬───────────────┘
                 │                                    │
                 ▼                                    ▼
   ┌─────────────────────────────────┐  ┌─────────────────────────────┐
   │ render2d::begin_frame           │  │ audio::tick (kira backend)  │
   │ - clear                          │  │ - update buses              │
   │ - draw background                │  │ - advance fades             │
   │ - draw walkers                   │  │ - pump subtitle queue       │
   │ - draw props                     │  └─────────────────────────────┘
   │ - draw light blends              │
   │ - draw HUD / inventory           │
   │ - draw post overlay              │
   │ render2d::end_frame (submit)     │
   └─────────────────────────────────┘
                 │
                 ▼
   ┌─────────────────────────────────┐
   │ winit present (vsync)            │
   └─────────────────────────────────┘
```

## Asset resolution flow

```
   author writes RON/PNG/OGG     →    assets/forest.scene
                                         │
   dev loader (notify-watched)   →    AssetRecord added to manifest.toml
                                         │
                                         ▼
   scene asks for AssetId("forest.scene")
                                         │
                                         ▼
   assets::Loader::load(AssetId)  →    Future<Output = Arc<dyn Asset>>
                                         │
   - check Arc<OnceCell> cache            │
   - if miss:                             │
       - tokio::spawn read bytes          │
       - resolve deps recursively         │
       - deserialize (serde RON/binary)   │
       - store in OnceCell                │
   - return Arc clone                     │
                                         │
                                         ▼
   consumer (render2d, audio, scene) holds Arc<T>
                                         │
   when last Arc drops  →  weak in LRU  →  evictable
```

## Save / load flow

```
   trigger (manual or checkpoint)
                │
                ▼
   save::Builder::new()
       .magic(0x53415647)
       .engine_version(semver)
       .schema("adventure-save-v1")
       .custom_version(SAVE_SCHEMA_GUID, 1)
       .snapshot(world, |s| {
           s.tags(&state.tags);
           s.inventory(&inventory);
           s.var_table(&state.vars);
           s.current_scene(scene_id, node_id);
       })
       .to_bytes()
                │
                ▼
   write to saves/slot1.bin
                │
   ─────── load later ───────
                │
                ▼
   save::Reader::from_bytes(bytes)?
       .check_magic()?
       .check_engine_version_compatible()?
       .apply_migrations()?;
                │
                ▼
   apply snapshot to fresh world
```

## Cross-crate conventions

- **`AssetId`** is defined in `core` and used everywhere — no crate has its own asset-handle type.
- **`Error`** is defined in `core` with `#[from]` for each common error (Io, Decode, Script, SaveVersion). All crates return `core::Result<T>`.
- **`Tag`** is defined in `state` but its interned string storage is shared via `core::Interner`.
- **RON** is the only data format used in `assets/` manifests and `scene/` files. `rmp-serde` is used only for `save/`.
- **No crate has `unsafe`** except `render2d` (wgpu FFI boundaries) and `audio` (kira backend).

## What's intentionally NOT in the graph

- No `net` crate — single-player only.
- No `physics` crate — point-and-click doesn't need it.
- No `editor` crate — replaced by `tools/inspector` (a separate binary).
- No `script_vm` crate — Rhai is the only scripting surface.
- No `material_editor` — 4 hardcoded WGSL shaders cover everything.
