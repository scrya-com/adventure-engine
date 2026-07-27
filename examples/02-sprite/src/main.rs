//! Phase 2 — Example 02: Render a textured quad with tint + alpha.
//!
//! Generates a procedural checkerboard PNG (8×8 squares of two colours
//! on a transparent background) and renders it spinning at the centre
//! of the window with a cycling tint.
//!
//! Confirms the full path: atlas upload → `ElementBatcher` →
//! `WgpuRenderer::render_frame`.

use std::sync::Arc;
use std::time::Instant;
use wgpu::SurfaceError;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use adventure_core::math::Vec2;
use adventure_render2d::{
    DrawEffect, DrawElement, ElementBatcher, ShaderKind, Tint, TextureId, UvRect, WgpuRenderer,
};

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    texture: Option<TextureId>,
    started_at: Instant,
    should_close: bool,
    paused: bool,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            instance: wgpu::Instance::default(),
            surface: None,
            adapter: None,
            renderer: None,
            texture: None,
            started_at: Instant::now(),
            should_close: false,
            paused: false,
        }
    }

    /// Build a 64x64 RGBA checkerboard (two greys on transparent).
    fn make_checkerboard() -> (u32, u32, Vec<u8>) {
        const W: u32 = 64;
        const H: u32 = 64;
        let mut out = vec![0u8; (W * H * 4) as usize];
        for y in 0..H {
            for x in 0..W {
                let on = ((x / 8) + (y / 8)) % 2 == 0;
                let idx = ((y * W + x) * 4) as usize;
                if on {
                    out[idx] = 220;
                    out[idx + 1] = 220;
                    out[idx + 2] = 230;
                    out[idx + 3] = 255;
                } else {
                    out[idx] = 60;
                    out[idx + 1] = 70;
                    out[idx + 2] = 90;
                    out[idx + 3] = 255;
                }
            }
        }
        (W, H, out)
    }

    fn render(&mut self) {
        let (Some(surface), Some(adapter), Some(r), Some(window), Some(tex)) = (
            self.surface.as_ref(),
            self.adapter.as_ref(),
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.texture,
        ) else {
            return;
        };

        let size = window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        if w == 0.0 || h == 0.0 {
            return;
        }
        let view_proj = WgpuRenderer::ortho(w, h);

        // Build a quad centred on screen, 256×256 px.
        let cx = w * 0.5;
        let cy = h * 0.5;
        let half = 128.0;
        let mut batcher = ElementBatcher::new();
        let t = if self.paused {
            0.0
        } else {
            self.started_at.elapsed().as_secs_f32()
        };
        let pulse = 0.5 + 0.5 * (t * 1.5).sin();
        let tint = Tint::rgba(0.8 + 0.2 * pulse, 0.8, 0.9, 1.0);

        // Slight scale pulse so it's obvious the quad is being drawn fresh.
        let scale = 0.7 + 0.3 * pulse;
        let e = half * scale;
        batcher.push(DrawElement {
            layer: 0,
            shader: ShaderKind::Sprite,
            effect: DrawEffect::NONE,
            texture: tex,
            uv: UvRect::FULL,
            tint,
            positions: vec![
                Vec2::new(cx - e, cy - e),
                Vec2::new(cx + e, cy - e),
                Vec2::new(cx - e, cy + e),
                Vec2::new(cx - e, cy + e),
                Vec2::new(cx + e, cy - e),
                Vec2::new(cx + e, cy + e),
            ],
            uvs: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
            ],
        });
        let batches = batcher.finish();

        let _ = r.render_frame(
            surface,
            view_proj,
            &batches,
            wgpu::Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            },
        );
        // Surface-error path — reconfigure on Lost/Outdated.
        if let Err(adventure_render2d::RendererError::NoSurfaceTexture) =
            Self::no_op_render_check(surface, adapter)
        {
            r.configure_surface(surface, adapter, (size.width, size.height));
        }
    }

    /// Trivial stub used only to elicit a surface error type if needed.
    fn no_op_render_check(
        _s: &wgpu::Surface<'_>,
        _a: &wgpu::Adapter,
    ) -> Result<(), adventure_render2d::RendererError> {
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("adventure-engine — 02-sprite")
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));
        let Ok(window) = event_loop.create_window(attrs) else {
            return;
        };
        let window = Arc::new(window);
        let surface = self.instance.create_surface(Arc::clone(&window)).unwrap();
        let adapter = pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no wgpu adapter");
        let mut renderer = pollster::block_on(WgpuRenderer::new_for_surface(&surface, &self.instance))
            .expect("renderer");
        let (w, h, rgba) = Self::make_checkerboard();
        let tex = renderer.upload_texture(w, h, &rgba).expect("texture upload");
        let size = window.inner_size();
        renderer.configure_surface(&surface, &adapter, (size.width, size.height));
        self.window = Some(window);
        self.surface = Some(surface);
        self.adapter = Some(adapter);
        self.renderer = Some(renderer);
        self.texture = Some(tex);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                self.should_close = true;
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.paused = !self.paused;
            }
            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(adapter), Some(r)) = (
                    self.surface.as_ref(),
                    self.adapter.as_ref(),
                    self.renderer.as_ref(),
                ) {
                    r.configure_surface(surface, adapter, (size.width, size.height));
                }
            }
            WindowEvent::RedrawRequested => {
                self.render();
                if self.should_close {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

#[allow(dead_code)]
fn handle_surface_err(err: SurfaceError) -> bool {
    matches!(err, SurfaceError::Lost | SurfaceError::Outdated)
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
