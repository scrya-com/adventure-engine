//! 2D renderer built on wgpu + a Slate-style element batcher.
//!
//! Mirrors UE's [`FSlateElementBatcher`](SlateCore/Rendering/ElementBatcher.h)
//! + [`ESlateShader`](SlateCore/Rendering/RenderingCommon.h:59) enum.
//! See `docs/RENDERING.md`.
//!
//! ## Layers
//!
//! * [`element`] — `DrawElement`, `SortKey`, `TextureId`, `Tint`, `UvRect`
//! * [`effect`] — `DrawEffect` bitflags (`ESlateDrawEffect`)
//! * [`shader`] — `ShaderKind` (`ESlateShader`)
//! * [`batcher`] — `ElementBatcher` (groups authored elements)
//! * [`atlas`] — `TextureAtlas` (guillotiere sub-allocator)
//! * [`renderer`] — `WgpuRenderer` (wgpu pipelines + frame lifecycle)

#![deny(missing_docs)]

pub mod atlas;
pub mod batcher;
pub mod effect;
pub mod element;
pub mod renderer;
pub mod shader;
mod shader_source;

pub use atlas::{AtlasError, TextureAtlas};
pub use batcher::{Batch, ElementBatcher};
pub use effect::DrawEffect;
pub use element::{DrawElement, SortKey, TextureId, Tint, UvRect};
pub use renderer::{FrameUniforms, RendererError, WgpuRenderer};
pub use shader::ShaderKind;
