# ADR 0002 — wgpu directly, not bevy_render

**Status:** Accepted
**Date:** 2026-07-27
**Decision maker:** John D. Pope

## Context

The rendering layer needs to choose between:

1. **`wgpu` directly** — build a thin Slate-style element batcher on top (~1500 LOC of our own code).
2. **`bevy_render`** — use Bevy's abstraction on top of wgpu.
3. **A higher-level 2D crate** — `macroquad`, `easyphase`, `bevy_2d`.

The UE 5.8 analysis (see `docs/DESIGN.md`) showed that the abstractions we actually want — `FSlateDrawElement` shape, `ESlateShader` enum, `ESlateDrawEffect` bitflags, `FSlateElementBatcher` sort-key grouping — are *above* the RHI layer. wgpu gives us RHI; we add a small layer on top.

## Decision

**Use wgpu directly.** Build a `crates/render2d/` layer (~1500 LOC target) that mirrors Slate's element batcher design. Do **not** use `bevy_render`, `bevy_2d`, `macroquad`, or any umbrella 2D framework.

## Rationale

1. **wgpu's abstraction shape is exactly UE's RHI.** `FRHITexture → wgpu::Texture`, `FRHIBuffer → wgpu::Buffer`, `FRHIGraphicsPipelineState → wgpu::RenderPipeline`, `FRHICommandList → wgpu::CommandEncoder`. The mapping is 1:1 (see `docs/RENDERING.md` table). Building on wgpu gives us the same conceptual layer as UE without 20 years of accreted forward-compat shims.
2. **Bevy_render brings a worldview we don't want.** Bevy assumes ECS-driven render extraction, app-driven plugin systems, and the Bevy reflect/asset pipelines. Our engine uses `bevy_ecs` (without the rest of bevy — see ADR 0003) for the world, but we don't want Bevy's render graph, render stages, or render app. Mixing them would create friction at every step.
3. **macroquad's API is too implicit.** macroquad uses a global context; calling `draw_texture(...)` writes to implicit state. This is great for game jams but painful when we need hotspot picking with custom cursor states, light-cone multiply blends, and a full-screen post overlay — all in the same frame with explicit ordering. Slate-style explicit `DrawElement` records + `ElementBatcher` sort give us that control.
4. **The shader catalog is tiny.** 4 WGSL shaders cover point-and-click: sprite tinted, multiply (light cones), overlay (atmosphere), post (full-screen). Bevy's material system is overkill.
5. **Atlas packing is independent.** `guillotiere` does the packing (mirrors UE's `SlateRHITextureAtlas`). `glyphon` does text. Neither requires bevy.
6. **Hot reload.** WGSL files are watched by `notify`. On change, recompile the `wgpu::ShaderModule`, swap pipelines. No engine restart.
7. **Future-proofing.** When we eventually want compute shaders (e.g., for softwareocclusion or GPU-driven light baking), wgpu exposes them directly. Going through bevy_render would add a translation layer.

## Consequences

- **Positive:** Full control over blend states, layering, and pipeline selection. Small dep surface (wgpu + guillotiere + glyphon + image + bitflags). Familiar shape for anyone who's worked with DirectX 12, Vulkan, or Metal directly.
- **Negative:** We write ~1500 LOC ourselves for the batcher. We handle pipeline barriers manually (though wgpu is largely implicit here). We don't benefit from bevy_render's optimization work.
- **Neutral:** WGSL is the shader language, not HLSL or GLSL. Compiles in milliseconds; reflective; easy to write.

## Alternatives considered

**`bevy_render`**: Rejected because it brings Bevy's ECS extraction model, render stages, and plugin system — all of which conflict with our hand-rolled `crates/engine/` design. Picking it would mean committing to all of Bevy.

**`macroquad`**: Rejected because the global-context API doesn't support per-element blend state and explicit layer ordering, which point-and-click needs for hotspot overlays + light cones + post effects.

**`easyphase` / `surfman`**: Rejected — surfman is mostly abandoned; easyphase is too niche.

## References

- `docs/RENDERING.md` — full design
- `docs/DESIGN.md` — UE 5.8 analysis (Slate element batcher section)
- SlateCore/Rendering/ElementBatcher.h:245-290 (UE reference)
