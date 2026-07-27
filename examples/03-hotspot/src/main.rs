//! Phase 3 — Example 03: Two hotspots; hover changes cursor, click logs.
//!
//! Defines two square hotspots in pixel space:
//!   * "door"   — exit hotspot at (100, 100)..(300, 300), cursor "walk"
//!   * "look"   — examine hotspot at (500, 100)..(700, 300), cursor "look"
//!
//! The dispatcher reports hover changes + clicks. We render faint
//! outlines of each hotspot so you can see them, and log to the console
//! when clicks land.

use std::sync::Arc;
use std::time::Instant;

use adventure_core::math::Vec2;
use adventure_input::{
    Dispatcher, InputEvent, MouseButton,
};
use adventure_render2d::{
    DrawEffect, DrawElement, ElementBatcher, ShaderKind, Tint, TextureId, UvRect, WgpuRenderer,
};
use adventure_scene::hotspot::{Cursor, Hotspot, HotspotKind, OnClick};
use smol_str::SmolStr;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    texture: Option<TextureId>,
    dispatcher: Dispatcher,
    hotspots: Vec<Hotspot>,
    started_at: Instant,
    should_close: bool,
}

impl App {
    fn new() -> Self {
        let hotspots = vec![
            Hotspot {
                id: SmolStr::new("door"),
                kind: HotspotKind::Exit,
                polygon: vec![
                    Vec2::new(100.0, 100.0),
                    Vec2::new(300.0, 100.0),
                    Vec2::new(300.0, 300.0),
                    Vec2::new(100.0, 300.0),
                ],
                cursor: Cursor::Named(SmolStr::new("walk")),
                on_click: OnClick::Action(SmolStr::new("exit_to_forest")),
            },
            Hotspot {
                id: SmolStr::new("look"),
                kind: HotspotKind::Examine,
                polygon: vec![
                    Vec2::new(500.0, 100.0),
                    Vec2::new(700.0, 100.0),
                    Vec2::new(700.0, 300.0),
                    Vec2::new(500.0, 300.0),
                ],
                cursor: Cursor::Named(SmolStr::new("look")),
                on_click: OnClick::Action(SmolStr::new("examine_painting")),
            },
        ];
        Self {
            window: None,
            instance: wgpu::Instance::default(),
            surface: None,
            adapter: None,
            renderer: None,
            texture: None,
            dispatcher: Dispatcher::new(),
            hotspots,
            started_at: Instant::now(),
            should_close: false,
        }
    }

    fn render(&mut self) {
        let (Some(surface), Some(_adapter), Some(r), Some(window), Some(tex)) = (
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

        // Draw the two hotspots as outlined quads (just outlines — 4 line
        // loops; we'll fake an outline as a thin tinted quad for simplicity).
        let mut batcher = ElementBatcher::new();
        let t = self.started_at.elapsed().as_secs_f32();
        for hotspot in &self.hotspots {
            let hovered = self.dispatcher.state().mouse_pos();
            let inside = point_in_polygon(hovered, &hotspot.polygon);
            let pulse = if inside { 0.5 } else { 0.15 + 0.1 * (t * 2.0).sin() };
            let tint = Tint::rgba(pulse, pulse, 0.9, 0.4);
            let p = &hotspot.polygon;
            // Quad: TL, TR, BR, TL, BR, BL
            batcher.push(DrawElement {
                layer: 0,
                shader: ShaderKind::Sprite,
                effect: DrawEffect::NONE,
                texture: tex,
                uv: UvRect::FULL,
                tint,
                positions: vec![p[0], p[1], p[2], p[0], p[2], p[3]],
                uvs: vec![
                    Vec2::ZERO,
                    Vec2::new(1.0, 0.0),
                    Vec2::new(1.0, 1.0),
                    Vec2::ZERO,
                    Vec2::new(1.0, 1.0),
                    Vec2::new(0.0, 1.0),
                ],
            });
        }
        let batches = batcher.finish();

        let _ = r.render_frame(
            surface,
            view_proj,
            &batches,
            wgpu::Color {
                r: 0.04,
                g: 0.04,
                b: 0.06,
                a: 1.0,
            },
        );
    }
}

fn point_in_polygon(p: Vec2, polygon: &[Vec2]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = polygon.len();
    let mut j = n - 1;
    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];
        if ((pi.y > p.y) != (pj.y > p.y))
            && (p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y + f32::EPSILON) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("adventure-engine — 03-hotspot")
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
        let (w, h, rgba) = (1u32, 1u32, vec![255u8, 255, 255, 255]);
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
        // Translate winit → InputEvent and enqueue.
        if let Some(ie) = adventure_input::winit_adapter::from_winit(&event, None) {
            // Patch in the current mouse pos for MouseDown/Up events.
            let ie = match (ie, &event) {
                (
                    InputEvent::MouseDown {
                        button,
                        ..
                    },
                    WindowEvent::MouseInput { .. },
                ) => {
                    let pos = self.dispatcher.state().mouse_pos();
                    InputEvent::MouseDown { button, pos }
                }
                (
                    InputEvent::MouseUp {
                        button,
                        ..
                    },
                    WindowEvent::MouseInput { .. },
                ) => {
                    let pos = self.dispatcher.state().mouse_pos();
                    InputEvent::MouseUp { button, pos }
                }
                (other, _) => other,
            };
            self.dispatcher.enqueue(ie);

            // Process the queue.
            let outcome = self.dispatcher.flush(&self.hotspots, &mut []);
            if let Some(hovered) = &outcome.hovered_hotspot {
                tracing::info!("hover: {hovered} (cursor: {})", outcome.cursor.0);
            }
            for (button, hotspot_id) in &outcome.clicks {
                tracing::info!("click {} on hotspot: {hotspot_id}", fmt_button(button));
            }
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                self.should_close = true;
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

fn fmt_button(b: &MouseButton) -> &'static str {
    match b {
        MouseButton::Left => "Left",
        MouseButton::Right => "Right",
        MouseButton::Middle => "Middle",
        MouseButton::Other(_) => "Other",
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive(
            tracing::Level::INFO.into(),
        ))
        .with_max_level(tracing::Level::INFO)
        .init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
