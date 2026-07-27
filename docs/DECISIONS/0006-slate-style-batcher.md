# ADR 0006 — Slate-style ElementBatcher for rendering

**Status:** Accepted
**Date:** 2026-07-27
**Decision maker:** John D. Pope

## Context

The `crates/render2d/` layer needs an internal design. UE 5.8's analysis (see `docs/DESIGN.md`) showed that Unreal's Slate element batcher is the load-bearing 2D rendering abstraction. Should we:

1. **Port Slate's design** — `DrawElement` record, `ElementBatcher` with sort-key grouping, `ESlateShader` enum (4 of 11 variants), `ESlateDrawEffect` bitflags.
2. **Use a traditional sprite batcher** — single command buffer per texture, no layers.
3. **Use retained-mode UI rendering** — every draw call issued immediately.

## Decision

**Port Slate's design.** Build `crates/render2d/` around the `DrawElement + ElementBatcher + (layer, pipeline, texture, blend)` sort key model.

## Rationale

1. **The design has shipped thousands of games' UIs.** Slate is what Unreal Engine uses for all editor UI and most in-game UI. It's battle-tested.
2. **Sort-key batching minimizes pipeline switches.** Drawing 100 sprites on 5 textures in 3 layers naively is 100 draw calls. After batching by `(layer, pipeline, texture)`, it's ≤15 draw calls (5 textures × 3 layers). State changes are the slowest thing on modern GPUs; this eliminates most of them.
3. **The API is naturally data-driven.** A `DrawElement` is a struct, not a function call. You can build a `Vec<DrawElement>` over the frame, sort once, batch, and submit. Easy to reason about, easy to test, easy to serialize for replay debugging.
4. **Layers map directly to game needs:**
   - Layer -10: background image
   - Layer 0: walkable sprites
   - Layer 10: foreground props
   - Layer 20: light blends (multiply)
   - Layer 30: atmospheric overlays
   - Layer 100: HUD
   - Layer 200: post effects
5. **4 shaders cover all use cases.** UE's `ESlateShader` has 11 variants, but 7 are for fonts (we use `glyphon` for that). The 4 we need: `Default` (sprite), `Custom` (multiply/overlay via different blend state), `PostProcess` (full-screen), and one for our light-cone soft-light blend.
6. **`DrawEffect` bitflags are simple.** `PreMultipliedAlpha`, `NoBlending`, `InvertAlpha`, `SegregatedAlpha` cover the alpha-handling variants. No need for a richer state object.
7. **The code is small.** ~1500 LOC total: `DrawElement` struct + `SortKey` + `ElementBatcher` (queue + sort + group) + the 4 WGSL shaders + wgpu plumbing. Less than a third of Slate's equivalent code.

## Design

```rust
// crates/render2d/src/element.rs
#[derive(Clone)]
pub struct DrawElement {
    pub sort_key: SortKey,
    pub dst: Rect,           // screen-space destination
    pub uv: Rect,            // atlas source
    pub tint: [f32; 4],
    pub effects: DrawEffect,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SortKey {
    pub layer: i32,
    pub pipeline: PipelineId,  // Sprite / Multiply / Overlay / Post
    pub texture: TextureId,
    pub blend: BlendMode,
}
```

```rust
// crates/render2d/src/batcher.rs
pub struct ElementBatcher {
    queue: Vec<DrawElement>,
}

impl ElementBatcher {
    pub fn push(&mut self, e: DrawElement) { self.queue.push(e); }

    pub fn flush(&mut self, ctx: &mut FrameContext) {
        self.queue.sort_by_key(|e| e.sort_key);
        let mut batcher = group_by_sort_key(&self.queue);
        while let Some(batch) = batcher.next() {
            ctx.draw_batch(batch);  // uploads vertices, sets pipeline, binds texture, draws
        }
        self.queue.clear();
    }
}
```

## Why not traditional sprite batcher

A traditional sprite batcher groups by texture only, not by `(layer, pipeline, texture, blend)`. This works for simple games but fails when:
- Multiple layers need different draw orders (background vs. foreground).
- Multiple pipelines are needed (sprite + light multiply + post overlay).
- Multiple blend states are needed within a pipeline (additive vs. alpha-blend particles).

Sort-key batching handles all of these uniformly.

## Why not retained-mode immediate rendering

Retained-mode immediate rendering (e.g., "draw this sprite NOW") has no batching. Every draw call is a separate wgpu command. For a scene with 50 sprites + 10 light blends + HUD, that's 60+ draw calls per frame, most of them redundantly setting the same pipeline and texture.

The queue-then-flush pattern of Slate (and our port) eliminates this.

## Consequences

- **Positive:** Minimal state changes, easy to reason about, supports arbitrary layering and blend modes.
- **Positive:** The `DrawElement` struct is serializable; we can record a frame's draw list and replay it offline for debugging.
- **Negative:** One frame of latency between push and submit (because the sort happens at end-of-frame). Not visible to users, but worth knowing.
- **Neutral:** The batcher owns a `Vec<DrawElement>` that grows per frame. We reuse the allocation across frames (call `queue.clear()` not `queue = Vec::new()`).

## References

- `docs/RENDERING.md` — full design
- SlateCore/Rendering/ElementBatcher.h:245-290 (UE reference)
- SlateCore/Rendering/DrawElementTypes.h:45 (`FSlateDrawElement`)
- SlateCore/Rendering/RenderingCommon.h:59,90 (`ESlateShader`, `ESlateDrawEffect`)
