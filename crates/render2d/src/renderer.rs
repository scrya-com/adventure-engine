//! `WgpuRenderer` — wgpu pipelines + frame lifecycle.
//!
//! Surface-agnostic: the caller creates a [`wgpu::Surface`] (typically
//! from a `winit` window) and hands it to [`WgpuRenderer::new_for_surface`].
//!
//! Reference: SlateRHIRenderer/SlateRHIRenderer.cpp. We collapse UE's
//! multiple indirection layers into a single struct that owns:
//!   * the wgpu instance/adapter/device/queue
//!   * the per-shader pipeline cache
//!   * the per-texture bind-group cache
//!   * the texture table

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::atlas::TextureAtlas;
use crate::batcher::ElementBatcher;
use crate::element::TextureId;
use crate::shader::ShaderKind;

/// Errors from the renderer.
#[derive(Debug, Error)]
pub enum RendererError {
    /// Adapter request failed (no GPU / no compatible driver).
    #[error("wgpu adapter request failed")]
    NoAdapter,
    /// Device request failed (driver internal).
    #[error("wgpu device request failed: {0}")]
    NoDevice(String),
    /// Could not get a current surface texture (window closed, etc.).
    #[error("no current frame target")]
    NoSurfaceTexture,
    /// Image file failed to decode.
    #[error("image: {0}")]
    Image(String),
    /// Texture id is not in the table.
    #[error("unknown texture {0:?}")]
    UnknownTexture(TextureId),
    /// RGBA payload does not match the texture byte size.
    #[error("texture {id:?} size mismatch: expected {expected} bytes, got {got}")]
    TextureSize {
        /// Texture that was written.
        id: TextureId,
        /// Expected RGBA byte count.
        expected: usize,
        /// Payload length.
        got: usize,
    },
}

/// Per-frame uniform passed at bind group 0 of every shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct FrameUniforms {
    /// Orthographic `view_proj` (camera → clip) as a column-major mat4.
    pub view_proj: [f32; 16],
}

impl FrameUniforms {
    /// Build from a `Mat4`.
    pub fn from_view_proj(m: Mat4) -> Self {
        let mut out = [0.0f32; 16];
        for (i, &v) in m.to_cols_array().iter().enumerate() {
            out[i] = v;
        }
        Self { view_proj: out }
    }
}

/// GPU vertex format — matches the WGSL `VsIn`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GpuVertex {
    /// Pixel-space XY.
    pub pos: [f32; 2],
    /// Texture-normalized UV.
    pub uv: [f32; 2],
    /// Linear RGBA tint.
    pub tint: [f32; 4],
}

impl GpuVertex {
    const ATTRS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x2,
        1 => Float32x2,
        2 => Float32x4,
    ];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

/// Preferred surface format for this renderer.
pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

/// Preferred texture-upload format (RGBA).
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// One entry in the texture table.
pub(crate) struct TextureEntry {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    #[allow(dead_code)]
    view: wgpu::TextureView,
    bind: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// The wgpu renderer.
pub struct WgpuRenderer {
    /// Public so callers can do custom GPU work (e.g. compute).
    pub device: wgpu::Device,
    /// Public for `queue.submit_texture(...)` style helpers.
    pub queue: wgpu::Queue,
    pub(crate) pipelines: [wgpu::RenderPipeline; ShaderKind::count()],
    pub(crate) uniform_layout: wgpu::BindGroupLayout,
    pub(crate) texture_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    pub(crate) pipeline_layout: wgpu::PipelineLayout,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) default_bind: wgpu::BindGroup,
    pub(crate) textures: Vec<Option<TextureEntry>>,
}

impl WgpuRenderer {
    /// Create a headless renderer (no surface). Used for tests + compute.
    pub async fn new_headless() -> Result<Self, RendererError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok_or(RendererError::NoAdapter)?;
        Self::from_adapter(&adapter).await
    }

    /// Create a renderer bound to a particular surface.
    pub async fn new_for_surface(
        surface: &wgpu::Surface<'_>,
        instance: &wgpu::Instance,
    ) -> Result<Self, RendererError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: false,
                compatible_surface: Some(surface),
            })
            .await
            .ok_or(RendererError::NoAdapter)?;
        Self::from_adapter(&adapter).await
    }

    async fn from_adapter(adapter: &wgpu::Adapter) -> Result<Self, RendererError> {
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("adventure-render2d device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| RendererError::NoDevice(e.to_string()))?;

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<FrameUniforms>() as u64),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render2d pipeline layout"),
            bind_group_layouts: &[&uniform_layout, &texture_layout],
            push_constant_ranges: &[],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("render2d sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let default_texture = Self::make_white_texel(&device);
        let default_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("default bind group"),
            layout: &texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&default_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Compile pipelines for all four shaders.
        let mut p: [Option<wgpu::RenderPipeline>; ShaderKind::count()] = Default::default();
        for (i, kind) in [
            ShaderKind::Sprite,
            ShaderKind::Multiply,
            ShaderKind::Overlay,
            ShaderKind::Post,
        ]
        .iter()
        .enumerate()
        {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("render2d {kind:?} shader")),
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(kind.wgsl())),
            });
            p[i] = Some(
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&format!("render2d {kind:?} pipeline")),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: "vs_main",
                        buffers: &[GpuVertex::layout()],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: SURFACE_FORMAT,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                }),
            );
        }

        Ok(Self {
            device,
            queue,
            pipelines: [
                p[0].take().unwrap(),
                p[1].take().unwrap(),
                p[2].take().unwrap(),
                p[3].take().unwrap(),
            ],
            uniform_layout,
            texture_layout,
            pipeline_layout,
            sampler,
            default_bind,
            textures: Vec::new(),
        })
    }

    /// Configure a surface to a chosen size + the renderer's preferred
    /// format. Idempotent — safe to call every time the window resizes.
    pub fn configure_surface(
        &self,
        surface: &wgpu::Surface<'_>,
        adapter: &wgpu::Adapter,
        (w, h): (u32, u32),
    ) {
        let caps = surface.get_capabilities(adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|&f| f == SURFACE_FORMAT)
            .unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w.max(1),
            height: h.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);
    }

    /// Upload RGBA bytes as a new texture; returns its id.
    pub fn upload_texture(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<TextureId, RendererError> {
        let tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("render2d user texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TEXTURE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::default(),
            rgba,
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render2d texture bind"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let entry = TextureEntry {
            texture: tex,
            view,
            bind,
            width,
            height,
        };
        if self.textures.is_empty() {
            self.textures.push(None);
        }
        for (i, slot) in self.textures.iter_mut().enumerate() {
            if slot.is_none() && i != 0 {
                *slot = Some(entry);
                return Ok(TextureId(i as u32));
            }
        }
        self.textures.push(Some(entry));
        Ok(TextureId((self.textures.len() - 1) as u32))
    }

    /// Decode an image file (png/jpeg/webp) and upload it.
    pub fn upload_image_path(
        &mut self,
        path: &std::path::Path,
    ) -> Result<TextureId, RendererError> {
        let img = image::open(path).map_err(|e| RendererError::Image(e.to_string()))?;
        let rgba = img.to_rgba8();
        self.upload_texture(rgba.width(), rgba.height(), &rgba)
    }

    /// Pixel size of an uploaded texture.
    pub fn texture_size(&self, id: TextureId) -> Option<(u32, u32)> {
        self.textures
            .get(id.0 as usize)
            .and_then(|s| s.as_ref())
            .map(|e| (e.width, e.height))
    }

    /// Overwrite an existing texture with tightly packed RGBA8 (`width * height * 4`).
    ///
    /// Used by looping Movie() playback to upload the next decoded frame.
    pub fn update_texture(&self, id: TextureId, rgba: &[u8]) -> Result<(), RendererError> {
        let entry = self
            .textures
            .get(id.0 as usize)
            .and_then(|s| s.as_ref())
            .ok_or(RendererError::UnknownTexture(id))?;
        let expected = (entry.width as usize)
            .saturating_mul(entry.height as usize)
            .saturating_mul(4);
        if rgba.len() != expected {
            return Err(RendererError::TextureSize {
                id,
                expected,
                got: rgba.len(),
            });
        }
        write_rgba(&self.queue, &entry.texture, entry.width, entry.height, rgba);
        Ok(())
    }

    fn make_white_texel(device: &wgpu::Device) -> wgpu::TextureView {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render2d default white texel"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TEXTURE_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        tex.create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Allocate a fresh atlas of a given size (caller manages packing).
    pub fn make_atlas(&self, width: u32, height: u32) -> TextureAtlas {
        TextureAtlas::new(width, height)
    }

    /// Build an orthographic view_proj for top-left origin pixel space.
    ///
    /// wgpu NDC is Y-up (`y = +1` at the top of the viewport). UI / winit
    /// pixel space is Y-down with origin at the top-left, so we invert the
    /// vertical axis: pixel `(0, 0)` → clip `(-1, +1)`, pixel
    /// `(width, height)` → clip `(+1, -1)`.
    pub fn ortho(width: f32, height: f32) -> Mat4 {
        // left, right, bottom, top — bottom > top flips Y for y-down pixels.
        Mat4::orthographic_rh(0.0, width, height, 0.0, -1.0, 1.0)
    }

    /// Build vertices for a list of batches (used by tests + the frame).
    pub(crate) fn build_vertices(batches: &[crate::batcher::Batch]) -> Vec<GpuVertex> {
        let mut out = Vec::new();
        for b in batches {
            for ((p, uv), t) in b.positions.iter().zip(b.uvs.iter()).zip(&b.tints) {
                out.push(GpuVertex {
                    pos: [p.x, p.y],
                    uv: [uv.x, uv.y],
                    tint: [t.x, t.y, t.z, t.w],
                });
            }
        }
        out
    }

    /// Build a `wgpu::RenderPass`-friendly slice of vertex data from
    /// a batcher snapshot. Callers then issue draws themselves.
    pub fn vertices_for(&self, batcher: &mut ElementBatcher) -> Vec<GpuVertex> {
        Self::build_vertices(&batcher.finish())
    }

    /// Render one frame to a surface. Builds uniforms + vertex buffer,
    /// walks the batches, and submits a single command buffer.
    ///
    /// `clear` is the colour used when no elements draw first. If you
    /// want to draw over a previous frame, use `render_frame_with`
    /// instead (not yet implemented; see Phase 3).
    pub fn render_frame(
        &mut self,
        surface: &wgpu::Surface<'_>,
        view_proj: Mat4,
        batches: &[crate::batcher::Batch],
        clear: wgpu::Color,
    ) -> Result<(), RendererError> {
        let frame = surface
            .get_current_texture()
            .map_err(|_| RendererError::NoSurfaceTexture)?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let uniforms = FrameUniforms::from_view_proj(view_proj);
        let uniform_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("render2d uniform"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let uniform_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render2d frame bind"),
            layout: &self.uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buf,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        // Build vertex buffer for the whole frame.
        let verts = Self::build_vertices(batches);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render2d frame encoder"),
            });

        if verts.is_empty() {
            // Just clear — no geometry.
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render2d clear-only"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        } else {
            let vert_buf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("render2d vertex"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render2d main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &uniform_bind, &[]);
            pass.set_vertex_buffer(0, vert_buf.slice(..));

            // Walk batches; switch pipeline + texture bind per batch boundary.
            let mut cursor = 0u32;
            for b in batches {
                let pipeline = &self.pipelines[b.key.shader as usize];
                pass.set_pipeline(pipeline);
                let bind = self
                    .textures
                    .get(b.key.texture as usize)
                    .and_then(|s| s.as_ref().map(|e| &e.bind))
                    .unwrap_or(&self.default_bind);
                pass.set_bind_group(1, bind, &[]);
                let n = b.positions.len() as u32;
                pass.draw(cursor..cursor + n, 0..1);
                cursor += n;
            }
        }

        self.queue.submit([encoder.finish()]);
        frame.present();
        Ok(())
    }
}

/// Upload tightly packed RGBA8, padding rows to wgpu's copy alignment.
fn write_rgba(queue: &wgpu::Queue, texture: &wgpu::Texture, width: u32, height: u32, rgba: &[u8]) {
    let unpadded = width * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded = unpadded.div_ceil(align) * align;
    let size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let layout = wgpu::ImageDataLayout {
        offset: 0,
        bytes_per_row: Some(padded),
        rows_per_image: Some(height),
    };
    let dest = wgpu::ImageCopyTexture {
        texture,
        mip_level: 0,
        origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    };
    if padded == unpadded {
        queue.write_texture(dest, rgba, layout, size);
        return;
    }
    let mut buf = vec![0u8; (padded * height) as usize];
    for y in 0..height {
        let src = (y * unpadded) as usize;
        let dst = (y * padded) as usize;
        buf[dst..dst + unpadded as usize].copy_from_slice(&rgba[src..src + unpadded as usize]);
    }
    queue.write_texture(dest, &buf, layout, size);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batcher::ElementBatcher;
    use crate::element::{DrawElement, TextureId, UvRect};
    use crate::shader::ShaderKind;
    use adventure_core::math::Vec2;
    use glam::Vec4;

    #[test]
    fn gpu_vertex_attrs_count() {
        assert_eq!(GpuVertex::ATTRS.len(), 3);
    }

    #[test]
    fn gpu_vertex_layout_stride() {
        let layout = GpuVertex::layout();
        // 2 + 2 + 4 floats * 4 bytes = 32 bytes
        assert_eq!(layout.array_stride, 32);
    }

    #[test]
    fn frame_uniforms_round_trip() {
        let m = WgpuRenderer::ortho(800.0, 600.0);
        let u = FrameUniforms::from_view_proj(m);
        assert!(u.view_proj.iter().any(|v| *v != 0.0));
    }

    #[test]
    fn vertex_builder_groups_by_batch() {
        let mut b = ElementBatcher::new();
        for layer in 0..3 {
            b.push(DrawElement {
                layer,
                shader: ShaderKind::Sprite,
                effect: crate::effect::DrawEffect::NONE,
                texture: TextureId::FIRST,
                uv: UvRect::FULL,
                tint: crate::element::Tint::IDENTITY,
                positions: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                ],
                uvs: vec![
                    Vec2::new(0.0, 0.0),
                    Vec2::new(1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                ],
            });
        }
        let batches = b.finish();
        assert_eq!(batches.len(), 3);
        let verts = WgpuRenderer::build_vertices(&batches);
        assert_eq!(verts.len(), 9);
    }

    #[test]
    fn ortho_maps_top_left_origin_to_ndc_top_left() {
        let m = WgpuRenderer::ortho(800.0, 600.0);
        // Pixel (0,0) → NDC top-left (-1, +1)
        let origin = m * Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!((origin.x + 1.0).abs() < 1e-4, "x={}", origin.x);
        assert!((origin.y - 1.0).abs() < 1e-4, "y={}", origin.y);
        // Pixel (width, height) → NDC bottom-right (+1, -1)
        let br = m * Vec4::new(800.0, 600.0, 0.0, 1.0);
        assert!((br.x - 1.0).abs() < 1e-4, "x={}", br.x);
        assert!((br.y + 1.0).abs() < 1e-4, "y={}", br.y);
        // Pixel mid-top edge stays high in NDC Y (not flipped back)
        let mid_top = m * Vec4::new(400.0, 0.0, 0.0, 1.0);
        assert!((mid_top.y - 1.0).abs() < 1e-4, "y={}", mid_top.y);
    }
}
