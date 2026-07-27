# Rendering — wgpu + Slate-style Element Batcher

> Deep dive on `crates/render2d/`. ~1500 LOC target. See `docs/DESIGN.md` for the high-level rationale.

## UE 5.8 reference

Unreal's rendering stack (analyzed across Engine/Source/Runtime/{RHI,Renderer,SlateCore,Slate,SlateRHIRenderer,Paper2D}/):

```
UMG / Slate widgets
        ↓
FSlateApplication → FSlateDrawBuffer
        ↓
FSlateWindowElementList                    (SlateCore/Rendering/DrawElements.h:67)
        ↓
FSlateDrawElement                          (DrawElementTypes.h:45)
        ↓
FSlateElementBatcher::AddElements          (ElementBatcher.h:245-290)
   sorts by (Layer, Shader, Texture, DrawEffects, PrimitiveType)
        ↓
FSlateRenderingPolicy → FSlateRHIRenderingPolicy
   picks PSO via template `TSlateElementPS<ESlateShader::Default, ...>`
        ↓
FSlateRHIRenderer::DrawWindow_RenderThread
        ↓
RHI: FRHICommandList → FRHIGraphicsPipelineState → FRHIRenderPassInfo
        ↓
D3D12RHI / VulkanRHI / MetalRHI
```

**What we port:** the Slate element batcher data model.
**What we skip:** the 3D scene path (deferred renderer, Lumen, Nanite, RayTracing), Paper2D's primitive scene proxy (which rides the 3D scene path — overkill for sprites), the render dependency graph, texture streaming, the Material Editor, hlslcc.

## Stack

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

## RHI → wgpu mapping

| UE RHI | UE file:line | wgpu |
|---|---|---|
| `FRHITexture` | RHIResources.h:2167 | `wgpu::Texture` |
| `FRHIBuffer` | RHIResources.h:1627 | `wgpu::Buffer` |
| `FRHISamplerState` | RHIResources.h:633 | `wgpu::Sampler` |
| `FRHIBlendState` / `RasterizerState` / `DepthStencilState` | RHIResources.h:658/641/648 | fields inside `wgpu::RenderPipelineDescriptor` |
| `FRHIVertexDeclaration` | RHIResources.h:687 | `wgpu::VertexState::buffers` |
| `FRHIGraphicsPipelineState` | RHIResources.h:1091 | `wgpu::RenderPipeline` |
| `FRHIVertexShader` / `FRHIPixelShader` | RHIResources.h:987/1005 | `wgpu::ShaderModule` (WGSL) |
| `FRHIRenderPassInfo` | RHIResources.h:5346 | `begin_render_pass` |
| `FRHICommandList` chain | RHICommandList.h:453→2493→3590→4388 | `wgpu::CommandEncoder` + `Queue::submit` |

The command-list inheritance in UE exists to model immediate vs deferred recording. wgpu merges this into a single `CommandEncoder` / `RenderPassEncoder` pair — sufficient for a 2D engine.

## Shader catalog

Unreal's `ESlateShader` has 11 variants (`Default, Border, GrayscaleFont, ColorFont, LineSegment, Custom, PostProcess, RoundedBox, SdfFont, MsdfFont, Dynamic`). Point-and-click needs **4**:

### 1. `sprite.wgsl`

The default. Textured quad × vertex tint, alpha blend.

```wgsl
struct VertexInput { @location(0) pos: vec2<f32>, @location(1) uv: vec2<f32> }
struct VertexOutput { @position pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) tint: vec4<f32> }
struct Uniforms { view_proj: mat4x4<f32> }

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

@vertex fn vs(in: VertexInput, @builtin(vertex_index) vi: u32) -> VertexOutput {
    // Per-instance data: dst_rect, tint, uv_rect
    var out: VertexOutput;
    out.pos = u.view_proj * vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.tint = vec4<f32>(1.0);  // replaced per-instance
    return out;
}

@fragment fn fs(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, in.uv) * in.tint;
}
```

Blend state: standard alpha (`src_factor = SrcAlpha`, `dst_factor = OneMinusSrcAlpha`).

### 2. `multiply.wgsl`

For light cones / soft-light multiply. Same vertex shader as `sprite.wgsl`. Fragment multiplies destination by source:

```wgsl
@fragment fn fs(in: VertexOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv);
    return vec4<f32>(s.rgb * in.tint.rgb, 1.0);  // alpha unused; premultiplied apply
}
```

Blend: `src_factor = DstColor`, `dst_factor = Zero` (multiply).

### 3. `overlay.wgsl`

Screen blend for atmospheric overlays (fog, mist):

```wgsl
@fragment fn fs(in: VertexOutput) -> @location(0) vec4<f32> {
    let s = textureSample(tex, samp, in.uv) * in.tint;
    return vec4<f32>(1.0 - (1.0 - s.rgb) * (1.0 - s.rgb), s.a);  // screen
}
```

Blend: `src_factor = One`, `dst_factor = OneMinusSrcAlpha` (premultiplied).

### 4. `post.wgsl`

Full-screen tint/vignette/CRT shader. One full-screen triangle (cheaper than a quad):

```wgsl
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VertexOutput {
    // Generate a triangle covering the screen
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0)
    );
    var uv = array<vec2<f32>, 3>(
        vec2<f32>(0.0, -2.0), vec2<f32>(0.0, 1.0), vec2<f32>(2.0, 1.0)
    );
    var out: VertexOutput;
    out.pos = vec4<f32>(pos[vi], 0.0, 1.0);
    out.uv = uv[vi];
    return out;
}
```

Post effects applied via uniform args (tint color, vignette falloff, scanline strength).

## DrawEffect bitflags

UE's `ESlateDrawEffect` (RenderingCommon.h:90):

```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DrawEffect: u32 {
        const NONE                = 0;
        const NO_BLENDING         = 1 << 0;
        const PRE_MULTIPLIED_ALPHA= 1 << 1;
        const INVERT_ALPHA        = 1 << 2;
        const SEGREGATED_ALPHA    = 1 << 3;
        const TINTED              = 1 << 4;
        const NO_TEXTURE          = 1 << 5;
    }
}
```

## ElementBatcher design

```rust
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SortKey {
    pub layer: i32,           // depth in scene
    pub pipeline: PipelineId, // Sprite / Multiply / Overlay / Post
    pub texture: TextureId,   // atlas page or standalone
    pub blend: BlendMode,     // for variants within a pipeline
}

pub struct ElementBatcher {
    queue: Vec<DrawElement>,
}

impl ElementBatcher {
    pub fn push(&mut self, element: DrawElement) { /* ... */ }

    pub fn flush(&mut self, ctx: &mut FrameContext) {
        self.queue.sort_by_key(|e| e.sort_key());
        for batch in group_by_sort_key(&self.queue) {
            ctx.draw_batch(batch);
        }
        self.queue.clear();
    }
}

#[derive(Clone)]
pub struct DrawElement {
    pub sort_key: SortKey,
    pub dst: Rect,
    pub uv: Rect,
    pub tint: [f32; 4],
    pub effects: DrawEffect,
}
```

Mirrors `FSlateElementBatcher::AddElements` (ElementBatcher.h:245-290) and `FSlateRenderBatch` grouping.

## Public API

```rust
pub trait Renderer2D {
    // Resource creation
    fn create_texture(&self, data: &[u8], w: u32, h: u32, fmt: TextureFormat) -> TextureId;
    fn create_atlas(&self, size: u32) -> AtlasId;            // SlateRHITextureAtlas model
    fn atlas_insert(&self, atlas: AtlasId, data: &[u8], w: u32, h: u32) -> Rect;

    // Frame
    fn begin_frame(&mut self) -> FrameContext;
    fn draw_sprite(&mut self, ctx: &mut FrameContext, sprite: SpriteHandle, dst: Rect,
                   tint: [f32; 4], blend: BlendMode, layer: i32);
    fn draw_light(&mut self, ctx: &mut FrameContext, light: SpriteHandle, dst: Rect);
    fn draw_fullscreen_effect(&mut self, ctx: &mut FrameContext, shader: PostShaderId, params: PostParams);
    fn end_frame(&mut self, ctx: FrameContext);
}
```

## Crates used

- `wgpu` 22.x — the only heavy dep
- `guillotiere` — atlas packing (mirrors `SlateRHITextureAtlas`)
- `glyphon` — text layout (for HUD)
- `bitflags` — DrawEffect
- `image` — PNG/WebP decode at load time
- `winit` — window/event source (used by `crates/input/` not here directly)

## What we explicitly skip

| Thing | Why |
|---|---|
| Deferred renderer (FDeferredShadingSceneRenderer) | 3D only |
| Lumen, Nanite, RayTracing, VSM | 3D only |
| Render Dependency Graph (FRDGBuilder) | wgpu's implicit barriers are enough for 2D |
| Texture streaming (FStreamableTextureResource) | Backgrounds fit in 50–200 MB VRAM |
| Material Editor + hlslcc | 4 hardcoded shaders cover the use cases |
| `FRDGBuilder` pass graph | Overkill; explicit pass ordering in `Renderer2D::end_frame` |
| Cluster GC for textures | Arc/Weak + LRU |
| Shader permutation explosion | Hand-pick 4 pipelines |

## Examples landing in phase 2

- `examples/01-window/main.rs` — opens a winit window, clears to a color, presents. ~80 LOC.
- `examples/02-sprite/main.rs` — loads a PNG via `image`, uploads to wgpu, draws with vertex tint. ~150 LOC.

## Validation

- Existing scene-engine tests are unaffected (locomotion is separate from rendering).
- `cargo run --example 02-sprite` displays the sprite.
- Visual check: alpha blending correct, tint multiplies, no flicker.
