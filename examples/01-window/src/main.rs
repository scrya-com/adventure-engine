//! Phase 2 — Example 01: Open a window and clear it to a colour.
//!
//! Demonstrates the bare wgpu + winit plumbing. Hue cycles over time so
//! it's obvious rendering is live. Press any key or close the window
//! to exit.

use std::sync::Arc;
use std::time::Instant;
use wgpu::SurfaceError;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<adventure_render2d::WgpuRenderer>,
    started_at: Instant,
    should_close: bool,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            instance: wgpu::Instance::default(),
            surface: None,
            adapter: None,
            renderer: None,
            started_at: Instant::now(),
            should_close: false,
        }
    }

    fn render(&mut self) {
        let (Some(surface), Some(adapter), Some(r), Some(window)) = (
            self.surface.as_ref(),
            self.adapter.as_ref(),
            self.renderer.as_ref(),
            self.window.as_ref(),
        ) else {
            return;
        };
        let frame = match surface.get_current_texture() {
            Ok(t) => t,
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                let size = window.inner_size();
                r.configure_surface(surface, adapter, (size.width, size.height));
                return;
            }
            Err(_) => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Cycle a calm hue over time.
        let t = self.started_at.elapsed().as_secs_f32();
        let clear = wgpu::Color {
            r: (0.15 + 0.10 * (t * 0.7).sin()) as f64,
            g: (0.20 + 0.10 * (t * 0.9).cos()) as f64,
            b: (0.30 + 0.10 * (t * 0.5).sin()) as f64,
            a: 1.0,
        };

        let mut encoder =
            r.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("01-window encoder"),
                });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("01-window clear"),
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
        }
        r.queue.submit([encoder.finish()]);
        frame.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("adventure-engine — 01-window")
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
        let renderer = pollster::block_on(adventure_render2d::WgpuRenderer::new_for_surface(
            &surface,
            &self.instance,
        ))
        .expect("renderer");
        let size = window.inner_size();
        renderer.configure_surface(&surface, &adapter, (size.width, size.height));
        self.window = Some(window);
        self.surface = Some(surface);
        self.adapter = Some(adapter);
        self.renderer = Some(renderer);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                self.should_close = true;
            }
            WindowEvent::Resized(size) => {
                if let (Some(surface), Some(adapter), Some(r)) =
                    (self.surface.as_ref(), self.adapter.as_ref(), self.renderer.as_ref())
                {
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

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
