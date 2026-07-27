# Adventure Engine — Design Document

> Synthesized from an architectural analysis of Unreal Engine 5.8 (Engine/Source/Runtime/, ~194 modules). This document explains *why* every load-bearing choice in this repo was made. New subsystems should reference this document; deviations belong in `docs/DECISIONS/`.

## TL;DR

Collapse Unreal's 194 runtime modules into ~16 Rust crates. Use **wgpu + a Slate-style element batcher** for rendering, **bevy_ecs** for the world, **serde + Arc/Weak** for assets and references, **data-driven RON + Rhai** for scripting, and **kira** for audio. Skip physics, networking, 3D rendering, the material editor, cooking, GC, and the Blueprint VM entirely.

## Recommended crate stack

| Concern | Crate | UE module replaced |
|---|---|---|
| Math | `glam` | Core/Math |
| Collections / interning | std + `string_interner` | Core/Containers |
| Logging | `tracing` | Core/Logging |
| Async runtime | `tokio` | Core/Async, Tasks |
| Reflection | `bevy_reflect` (optional) | CoreUObject |
| ECS | `bevy_ecs` (no umbrella) | Engine/Actor-Component |
| Rendering | `wgpu` + custom batcher (~1500 LOC) | RHI + Slate |
| Atlas packing | `guillotiere` | SlateRHITextureAtlas |
| Text | `glyphon` | Slate font rendering |
| Audio | `kira` (+ `lewton`/`audiopus`) | AudioMixer |
| Assets | `serde` + `rmp-serde` + `tokio` | AssetRegistry + AsyncLoading2 |
| File-watch | `notify` | Hot reload |
| Scripting | `rhai` | Blueprint VM |
| Save game | `serde` + versioned deserializer | USaveGame + FCustomVersion |
| Dialogue | custom RON (evaluate Yarn Spinner later) | CommonConversation plugin |
| Localization | `fluent` | FText |
| Windowing/events | `winit` | ApplicationCore |

## Module mapping — 194 → 16

| UE module group | Verdict | Rust crate |
|---|---|---|
| **Core, CoreUObject, Engine, Projects, ApplicationCore** | REQUIRED | `core`, `engine` |
| InputCore, InputDevice | REQUIRED | `input` |
| RHI, RHICore, RenderCore, Renderer | REPLACE | `render2d` (wgpu-based, ~1500 LOC) |
| SlateCore, Slate, SlateRHIRenderer, UMG | USEFUL (subset) | `ui` |
| AudioMixer, SignalProcessing, codec decoders | USEFUL | `audio` |
| AssetRegistry, PakFile | USEFUL (simplified) | `assets`, `tools/packer` |
| GameplayTags, GameplayTasks, LevelSequence | USEFUL | `state`, `cutscene` |
| Physics (Chaos, ClothingSystem, FieldSystem) | SKIP | — |
| Net, Networking, Sockets, Online, Iris | SKIP | — |
| Platform-specific (Linux/Windows/Mac/iOS/Android) | SKIP | winit/wgpu handle |
| Editor | SKIP | `tools/inspector` (RON) replaces it |
| AIModule, Navmesh, MassEntity | SKIP | point-and-click doesn't pathfind (we have `locomotion/` for character nav) |
| AR/HMD/EyeTracker, Landscape, Foliage | SKIP | — |
| Media, AVEncoder, AVIWriter | SKIP | pre-rendered via `render2d` |

## Workspace skeleton

```
adventure-engine/
├── crates/
│   ├── core/         glam, tracing, string_interner, async, Error
│   ├── locomotion/   FORK of scene-engine (walk graphs, plant FSM, retargeting)
│   ├── graph/        VENDORED aval_graph (ring-arc route planning)
│   ├── scene/        Scene/Room/Hotspot/Prop + RON format
│   ├── state/        GameplayTags + VarTable + StateTree FSM
│   ├── dialogue/     dialog trees + Rhai conditions
│   ├── inventory/    items + verb coin
│   ├── save/         versioned serde save format
│   ├── assets/       AssetRecord + async loader + Arc/Weak
│   ├── render2d/     wgpu + Slate-style batcher + 4 shaders
│   ├── audio/        kira 4-bus wrapper
│   ├── input/        winit → Interactive dispatch
│   ├── ui/           immediate-mode menus + retained HUD
│   ├── scripting/    Rhai wrapper
│   ├── cutscene/     timeline / sequencer
│   ├── localization/ fluent-rs
│   └── engine/       main loop, owns World + systems
├── examples/         numbered per phase
└── tools/            packer, inspector
```

## Layer designs

### Core / World

**Skip:** UE's entire UObject GC + UClass reflection + Actor-Component tree + Pawn/Character/Controller/MovementComponent stack.

**Keep the conceptual hierarchy**, map to ECS:
- `UWorld` → `bevy_ecs::World`
- `ULevel` → `Room` (a tagged set of entities)
- `AActor` → `Entity`
- `UActorComponent` → ECS `Component`
- `AGameMode` / `APlayerController` → Rust systems + a `GameMode` resource

```rust
#[derive(Component)]
struct Transform2D { pos: Vec2, rot: f32, scale: Vec2 }

#[derive(Component)]
struct Sprite { atlas: AssetId, region: Rect<i32>, layer: i32 }

#[derive(Component)]
struct Hotspot { polygon: Vec<Vec2>, cursor: CursorId, on_click: ActionId }

#[derive(Component)]
struct Walker { graph: WalkGraph, plant: PlantController }
```

**GC:** Rust's `Arc`/`Weak` eliminate mark-sweep entirely.

### Rendering

**Stack:**
```
draw_sprite / draw_light / draw_fullscreen   (FSlateDrawElement-shape API)
        ↓
ElementBatcher   (sort key: layer, pipeline, texture, blend)
        ↓
wgpu::RenderPipeline   (one per shader: Sprite, Multiply, LightBlend, PostOverlay)
        ↓
wgpu   (replaces FRHICommandList, FRHITexture, FRHIGraphicsPipelineState, …)
        ↓
platform surface   (winit)
```

**RHI → wgpu mapping** is essentially 1:1:
- `FRHITexture` → `wgpu::Texture`
- `FRHIBuffer` → `wgpu::Buffer`
- `FRHIGraphicsPipelineState` → `wgpu::RenderPipeline`
- `FRHICommandList` chain → `wgpu::CommandEncoder` + `Queue::submit`
- `FRHIRenderPassInfo` → `begin_render_pass`

**Just 4 shaders** (vs UE's thousands of permutations):
1. `sprite.wgsl` — textured quad × vertex tint, alpha blend (`ESlateShader::Default`)
2. `multiply.wgsl` — soft-light / multiply for light cones
3. `overlay.wgsl` — screen blend for atmospheric overlays
4. `post.wgsl` — full-screen tint/vignette/CRT (one full-screen triangle)

`ESlateDrawEffect` bitflags map to `bitflags!` (`PreMultipliedAlpha`, `NoBlending`, `InvertAlpha`, `SegregatedAlpha`).

**Skip entirely:** deferred renderer, Lumen, Nanite, RayTracing, render dependency graph, texture streaming, Material Editor, hlslcc. Adventure backgrounds fit in 50–200 MB VRAM — load whole at startup.

```rust
trait Renderer2D {
    fn create_texture(&self, data: &[u8], w: u32, h: u32, fmt: TextureFormat) -> TextureId;
    fn create_atlas(&self, ...);
    fn begin_frame(&mut self) -> FrameContext;
    fn draw_sprite(&mut self, ctx: &mut FrameContext, sprite: SpriteHandle, dst: Rect, tint: [f32;4], blend: BlendMode, layer: i32);
    fn draw_light(&mut self, ctx: &mut FrameContext, ...);
    fn draw_fullscreen_effect(&mut self, ctx: &mut FrameContext, shader: PostShaderId);
    fn end_frame(&mut self, ctx: FrameContext);
}
```

### Assets

**Lifecycle:**
```
IMPORT (PNG/OGG/RON source)  →  manifest.toml (dev) / manifest.bin (ship)
                                       ↓
                              async loader (threadpool)
                                       ↓
              AssetId → OnceCell<Arc<Asset>> dedupe + dep resolution
                                       ↓
                  Arc<Asset> (hard) | AssetId (soft) | Weak<Asset> (LRU-evictable)
```

**Manifest format** (mirrors `FAssetData` + `FPakEntry`):
```rust
struct AssetRecord {
    id: AssetId,                      // u64 path hash
    path: SmolStr,
    kind: AssetKind,                  // Background | Sprite | Audio | Scene | Dialog
    payload_offset: u64,
    payload_len: u64,
    uncompressed_len: u64,
    sha1: [u8; 20],                   // FPakEntry.Hash
    deps: SmallVec<[AssetId; 4]>,
    compression: Compression,         // None | LZ4 | Zstd
    chunk_id: u16,                    // chapter / DLC chunking
}
```

**Backgrounds** stream at the texture level (not per-asset):
```rust
struct Background { full_res: OnceCell<RgbaImage>, thumb: Arc<RgbaImage> }
```
Load `full_res` on scene enter, evict on exit (mirrors `UStreamableRenderAsset::StreamIn/StreamOut`).

**Cooking** collapses to a ~200-line packer: dep flatten + LZ4 + chapter chunking. No shader permutations, no DDC.

**Skip:** UObject reflection serialization, FArchive, AsyncLoading2 dependency walker (Rust's recursive Future + `OnceCell` covers it), IoStore content-addressing (only worth it past ~10 GB), cluster GC.

### Input + UI

**Click dispatch** (Unreal's chain collapses dramatically):
```
winit MouseInput                              (OS event)
    ↓
input::dispatch(MouseMove/Down/Up/Key/Char)  (one step vs UE's 6)
    ↓
pick(scene, mouse_pos) → ordered list of Interactive sprites
    ↓
Hotspot.on_hover_enter / on_click / cursor
```

UE needs `PlayerController::InputKey → GetHitResultAtScreenPosition → PrimitiveComponent::DispatchOnClicked` because of 3D line traces and replication. A 2D pick is one spatial query returning ordered sprites — no `PlayerController` intermediary.

**UI mode** — firm recommendation:
- **Retained** for the **world interaction layer** (hotspots, hover state machines, cursor state) — long-lived, event-driven.
- **Immediate** for **menus / inventory grids / dialog** — rebuilt from data each frame, no widget identity to persist.

This mirrors what UE actually does: `AHUD::HitBox` is effectively immediate-mode (re-registered every `DrawHUD`), while UMG is retained.

```rust
enum InputEvent {
    MouseMove(Vec2), MouseDown(Button), MouseUp(Button),
    Key(Key), Char(char), MouseWheel(f32),
}

trait Interactive {
    fn on_hover_enter(&mut self);
    fn on_hover_exit(&mut self);
    fn on_click(&mut self, btn: Button);
    fn cursor(&self) -> Option<Cursor>;
}
```

**Reference design** for dialog: `CommonConversation` plugin (Engine/Plugins/Experimental/CommonConversation/) — graph-driven, gameplay-tag-keyed, with `ConversationEntryPointNode` / `ChoiceNode` / `LinkNode` / `RequirementNode` / `SideEffectNode` / `TaskNode`. Use as reference; not a stable dependency.

### Scripting + State

**Scripting** — firm recommendation:
- **80% pure data-driven** — RON/YAML files describing click regions, dialog trees, item interactions. Hot-reloaded via `notify` file-watch. No VM, no GC.
- **20% Rhai** for arithmetic / conditions (`if score > 5 and has_key`). Rust-native, sandboxed, no `unsafe`, no GC, no FFI friction. Skip Lua.
- **Skip Unreal's Blueprint VM** entirely (`EExprToken` opcodes, `ProcessInternal` switch). It only existed to let non-programmers author in-editor graphs.

**Game loop** (mirrors `LaunchEngineLoop.cpp:5575`):
```
1. poll input events            (OnSamplingInput analog)
2. flush input → world          (PlayerController::InputKey analog)
3. world tick                   (UWorld::Tick — 2-3 groups, not 8)
   - TG_PrePhysics   → update_walkers, animations
   - TG_PostUpdate   → camera follow, scene transitions
4. submit render                (renderer::end_frame)
5. audio tick                   (kira)
6. end frame                    (vsync wait)
```

**Save system** (mirror `FSaveGameHeader` at `GameplayStatics.cpp:89`):
```rust
struct SaveHeader {
    magic: u32,                          // 0x53415647 ('SAVG') — UE uses the same
    engine_semver: (u16, u16, u16),
    content_version: u32,
    schema_name: SmolStr,
    custom_versions: SmallVec<[(Guid, u32); 4]>,  // FCustomVersionContainer analog
}
```
- Persist **world-state diff** only (tag set, inventory, var table, current scene_id + node_id) — not full actor serialization.
- `serde + rmp-serde` (~10× smaller than JSON).
- Version the deserializer, not the data — each migration lives in a `load_v3_to_v4` function.

**GameplayTags** for state flags (`State.NPC.Bob.Met`, `State.Door.Cellar.Open`) — hierarchical, indexed, queryable. Maps directly to a Rust `Tag` newtype wrapping an interned `&str` with `has` / `has_any` / `has_all` / `matches_query`.

**StateTree** (Plugins/Runtime/StateTree/) is the reference model for "current scene / current dialog node / current chapter" — hierarchical state machine with enter/exit/tick + per-asset `FStateTreeCustomVersion`.

### Audio

**3-bus model** (UE's `USoundClass` → Rust enum): `Master | Music | Sfx | Voice`.

```rust
trait AudioEngine {
    fn play_oneshot(&mut self, clip: ClipId, bus: Bus, vol: f32);     // UGameplayStatics::PlaySound2D
    fn play_looping(&mut self, clip: ClipId, bus: Bus) -> Handle;     // UAudioComponent + bLooping
    fn crossfade(&mut self, music: Handle, to: ClipId, secs: f32, curve: Curve);
    fn fade_bus(&mut self, bus: Bus, to_vol: f32, secs: f32);         // USoundMix push
    fn queue_vo(&mut self, clip: ClipId, subtitle: Subtitle) -> Handle;
    fn stop(&mut self, h: Handle, fade_secs: f32);
}
enum Curve { Linear, Logarithmic, EqualPower }  // EAudioFaderCurve
```

**Skip:** attenuation, spatialization (set `bSpatialize=false`), submix DSP graph, MetaSounds.

**Codecs:** Vorbis (music, via `lewton`) + Opus (VO, via `audiopus`). Pre-decode VO to WAV at build time if you want zero runtime codec deps.

## UE concept → Rust mapping cheat sheet

| UE concept | UE file | Rust equivalent |
|---|---|---|
| `UObject` reflection | CoreUObject/Object.h:98 | `bevy_reflect::Reflect` (optional) |
| `FName` interned string | Core/Containers/NameTypes.h | `string_interner::StringInterner` |
| `TArray`/`TMap`/`TSet` | Core/Containers/ | std Vec/HashMap/HashSet |
| `FArchive` serialization | Core/Serialization/Archive.h | `serde::Serialize/Deserialize` |
| `TSoftObjectPtr<T>` | CoreUObject/SoftObjectPtr.h:38 | `AssetId` (path-only, lazy) |
| `TStrongObjectPtr` | CoreUObject | `Arc<T>` |
| `TWeakObjectPtr` | CoreUObject/WeakObjectPtr.h | `Weak<T>` |
| Mark-sweep GC | CoreUObject/GarbageCollection.h | Rust ownership (none needed) |
| `UWorld`/`ULevel`/`AActor` | Engine/Classes/Engine/ | ECS World + tagged entity sets |
| `UActorComponent` | Components/ActorComponent.h:160 | ECS Component |
| `FGameplayTag` | GameplayTagContainer.h:41 | `Tag` newtype over interned `&str` |
| `FRHITexture`/`FRHIBuffer`/`FRHIGraphicsPipelineState` | RHI/RHIResources.h | `wgpu::Texture`/`Buffer`/`RenderPipeline` |
| `FSlateDrawElement` | SlateCore/Rendering/DrawElementTypes.h:45 | `DrawElement` record |
| `ESlateShader` (11 variants) | SlateCore/Rendering/RenderingCommon.h:59 | 4-variant Rust enum |
| `FSlateElementBatcher` | SlateCore/Rendering/ElementBatcher.h:245 | custom `ElementBatcher` (~1500 LOC) |
| `UAudioComponent` | Components/AudioComponent.h | `kira` track + handle |
| `USoundClass` (bus) | Sound/SoundClass.h | `Bus` enum |
| `USoundMix` (crossfade) | Sound/SoundMix.h | `fade_bus()` method |
| `UStreamableRenderAsset` | Engine/StreamableRenderAsset.h:37 | `Background { full_res: OnceCell<_>, thumb: Arc<_> }` |
| `FAssetData` | AssetRegistry/AssetData.h:200 | `AssetRecord` struct |
| `FPakEntry` | PakFile/IPlatformFilePak.h:405 | manifest entry + offset/len/sha1 |
| `USaveGame` + header | GameFramework/SaveGame.h, GameplayStatics.cpp:89 | versioned serde struct |
| Blueprint VM (`EExprToken`, `ProcessInternal`) | CoreUObject/Script.h:193, ScriptCore.cpp:1364 | **skip** — Rhai for the 20% that needs logic |
| StateTree | Plugins/Runtime/StateTree/ | state-machine crate or hand-rolled |
| Common Conversation | Plugins/Experimental/CommonConversation/ | `yarnspinner` or custom RON dialog trees |
| `MovieScreen`/`LevelSequence` | Runtime/MovieScene, LevelSequence | timeline/sequencer crate |

## Heritage integration

This engine wraps two existing pieces of code rather than reimplementing them:

### `crates/locomotion/` — forked from scene-engine

**What it provides** (3385 LOC across 8 files):
- `scene_point.rs` (56 LOC) — `ScenePoint` with x, y, depth (for perspective scale)
- `walk_graph.rs` (505 LOC) — `WalkGraph`, `WalkGraphNode`, Dijkstra pathfinding
- `ardy_motion.rs` (550 LOC) — animation clip + frame, JSON parse
- `path_retarget.rs` (826 LOC) — anti-moonwalk + stable-gait retargeting
- `plant.rs` (439 LOC) — `PlantController` FSM + stride timing (SE→NW moonwalk regression tested)
- `compass.rs` (213 LOC) — 8-dir walk ring + AVAL state resolve
- `verb.rs` (62 LOC) — `Verb` enum (Look/Use/Talk-to)
- `meta_action.rs` (61 LOC) — high-level intent vocabulary

**Tested invariants to preserve:**
- SE→NW moonwalk regression (`plant.rs:367`)
- Gait-locked plant timing (`plant.rs:258`)
- Stable gait window (`path_retarget.rs:621`)
- Ring hops (`plant.rs:258`)
- Compass bin parity (`compass.rs:158`)

### `crates/graph/` — vendored aval_graph

**What it provides** (634 LOC, single file):
- `RingDefinition`, `RingArc`, `TieBreak`
- `plan_ring_arc(ring, from, to)` — shorter arc on a ring
- `plan_state_hops` — multi-hop BFS over directed edge adjacency
- Prefers pure walk-ring paths when both ends are `walk_*`

Used by `crates/locomotion/` for ring math. Pure Rust, zero deps.

### AVAL integration (deferred)

`aval_graph` is vendored now. The full AVAL integration (`.avl` codec bundle decode for rendered character motion) is deferred — sprite-based characters cover MVP. When the engine needs rendered motion:
- Add `crates/avl_decode/` wrapping `flutter/rust/aval_decode` (H.264 via openh264, BSD-2-Clause)
- Author character motion as `.avl` bundles (idle, walk-n/s/e/w, turn, talk, interact states)
- See `docs/INTEGRATION-AVAL.md` for the full plan

## What this engine is NOT

- **Not a server.** No HTTP, no replication. If you ever need HTTP, it lives in `PresidentialDilema-FastApi`, never here.
- **Not 3D.** No deferred renderer, no GBuffer, no Lumen, no Nanite. Sprite + light blend + post overlay only.
- **Not networked.** Single-player. No Iris, no replication, no voice chat.
- **Not physics-driven.** No Chaos, no collisions, no rigidbodies. Point-and-click doesn't need them. Walking uses precomputed nav graphs from `locomotion/`.
- **Not programmable by end-users.** Rhai is for *designers authoring scenes*, not for *modders extending the game*. Ship a frozen asset set.

## Where to read more

- `docs/ARCHITECTURE.md` — crate dependency graph and data flow diagram
- `docs/ROADMAP.md` — phased delivery plan with commit map
- `docs/RENDERING.md` — wgpu + Slate-style batcher deep dive
- `docs/SCRIPTING.md` — Rhai integration design
- `docs/AUDIO.md` — kira 4-bus model
- `docs/SAVE.md` — versioned save format
- `docs/DATA-FORMATS.md` — RON schemas for scenes, dialog, manifest
- `docs/INTEGRATION-AVAL.md` — AVAL `.avl` decode plan (deferred)
- `docs/DECISIONS/` — ADRs explaining why each foundation choice was made
