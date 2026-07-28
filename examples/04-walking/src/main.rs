//! Phase 4 — Example 04: Click-to-walk with the locomotion fork.
//!
//! Builds a 4-node walk graph in the lower half of the window, spawns
//! a player entity at the centre, and routes clicks through the graph.
//! A teal triangle marks the player; the route is rendered as a faint
//! orange polyline.

use std::sync::Arc;
use std::time::{Duration, Instant};

use adventure_core::math::Vec2;
use adventure_engine::{
    FrameContext, FrameSchedule, PendingClick, Player, SceneGraph, Transform2D, Walker,
};
use adventure_locomotion::{
    ArdyFrame, ArdyMotion, ScenePoint, WalkGraph, WalkGraphNode, WalkNodeKind,
};
use adventure_render2d::{
    DrawEffect, DrawElement, ElementBatcher, ShaderKind, Tint, TextureId, UvRect, WgpuRenderer,
};
use bevy_ecs::prelude::World;
use smol_str::SmolStr;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    texture: Option<TextureId>,
    world: Option<World>,
    schedule: FrameSchedule,
    last_frame: Instant,
    should_close: bool,
    last_click_pixel: Option<Vec2>,
}

fn make_cycle() -> ArdyMotion {
    ArdyMotion::new(20.0, vec![ArdyFrame::new(0.5, 0.5, 0.5)], vec![])
}

fn build_world() -> World {
    let mut world = World::new();
    world.insert_resource(FrameContext::default());
    world.insert_resource(PendingClick::default());

    // 4-node walk graph: a diamond.
    let mk_node = |id: &str, x: f64, y: f64| WalkGraphNode {
        id: id.to_string(),
        point: ScenePoint::new(x, y, 0.0),
        label: None,
        kind: WalkNodeKind::Floor,
    };
    let g = WalkGraph {
        nodes: vec![
            mk_node("left", 0.20, 0.70),
            mk_node("right", 0.80, 0.70),
            mk_node("bottom", 0.50, 0.90),
            mk_node("top", 0.50, 0.50),
        ],
        edges: vec![
            (0, 1), // left-right
            (0, 2), // left-bottom
            (0, 3), // left-top
            (1, 2), // right-bottom
            (1, 3), // right-top
        ],
    };

    world.insert_resource(SceneGraph { graph: g });

    // Player starts at the top node.
    world.spawn((
        Player,
        Walker::idle(make_cycle()),
        Transform2D::at(Vec2::new(0.5 * 800.0, 0.50 * 600.0)),
    ));

    world
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
            world: Some(build_world()),
            schedule: FrameSchedule::default(),
            last_frame: Instant::now(),
            should_close: false,
            last_click_pixel: None,
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

        let mut batcher = ElementBatcher::new();

        // Draw the 4 walk nodes as faint magenta circles (well, quads).
        let graph = self.world.as_ref().unwrap().resource::<SceneGraph>();
        for node in &graph.graph.nodes {
            let cx = (node.point.x as f32) * w;
            let cy = (node.point.y as f32) * h;
            let half = 6.0;
            batcher.push(quad(cx, cy, half, Tint::rgba(0.9, 0.4, 0.9, 0.7), tex));
        }

        // Draw the edges as thin lines (we fake lines as thin quads).
        let tint_edge = Tint::rgba(0.5, 0.5, 0.6, 0.4);
        for &(a_idx, b_idx) in &graph.graph.edges {
            let a = &graph.graph.nodes[a_idx];
            let b = &graph.graph.nodes[b_idx];
            let ax = (a.point.x as f32) * w;
            let ay = (a.point.y as f32) * h;
            let bx = (b.point.x as f32) * w;
            let by = (b.point.y as f32) * h;
            batcher.push(thin_line(ax, ay, bx, by, tint_edge, tex));
        }

        // Draw the player as a teal triangle.
        let mut q = self.world.as_mut().unwrap().query::<(&Walker, &Transform2D)>();
        let (_walker, transform) = q.iter(self.world.as_ref().unwrap()).next().unwrap();
        let px = transform.pos.x;
        let py = transform.pos.y;
        drop(q);
        batcher.push(triangle(
            px,
            py,
            16.0,
            transform.facing,
            Tint::rgba(0.2, 0.9, 0.7, 0.95),
            tex,
        ));

        // Last-click marker.
        if let Some(click) = self.last_click_pixel {
            batcher.push(quad(click.x, click.y, 4.0, Tint::rgba(1.0, 0.8, 0.2, 0.9), tex));
        }

        let batches = batcher.finish();
        let _ = r.render_frame(
            surface,
            view_proj,
            &batches,
            wgpu::Color {
                r: 0.04,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            },
        );
    }
}

fn quad(cx: f32, cy: f32, half: f32, tint: Tint, tex: TextureId) -> DrawElement {
    DrawElement {
        layer: 0,
        shader: ShaderKind::Sprite,
        effect: DrawEffect::NONE,
        texture: tex,
        uv: UvRect::FULL,
        tint,
        positions: vec![
            Vec2::new(cx - half, cy - half),
            Vec2::new(cx + half, cy - half),
            Vec2::new(cx + half, cy + half),
            Vec2::new(cx - half, cy - half),
            Vec2::new(cx + half, cy + half),
            Vec2::new(cx - half, cy + half),
        ],
        uvs: quad_uvs(),
    }
}

fn thin_line(ax: f32, ay: f32, bx: f32, by: f32, tint: Tint, tex: TextureId) -> DrawElement {
    // 2px-wide quad along the segment.
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt().max(1e-4);
    let nx = -dy / len;
    let ny = dx / len;
    let thickness = 1.5_f32;
    let p1 = Vec2::new(ax + nx * thickness, ay + ny * thickness);
    let p2 = Vec2::new(bx + nx * thickness, by + ny * thickness);
    let p3 = Vec2::new(bx - nx * thickness, by - ny * thickness);
    let p4 = Vec2::new(ax - nx * thickness, ay - ny * thickness);
    DrawElement {
        layer: 0,
        shader: ShaderKind::Sprite,
        effect: DrawEffect::NONE,
        texture: tex,
        uv: UvRect::FULL,
        tint,
        positions: vec![p1, p2, p3, p1, p3, p4],
        uvs: quad_uvs(),
    }
}

fn triangle(
    cx: f32,
    cy: f32,
    size: f32,
    facing: adventure_scene::transform::Facing,
    tint: Tint,
    tex: TextureId,
) -> DrawElement {
    use adventure_scene::transform::Facing;
    // Tip points in the facing direction.
    let tip = match facing {
        Facing::West => Vec2::new(cx - size, cy),
        _ => Vec2::new(cx + size, cy),
    };
    let base_top = Vec2::new(cx, cy - size * 0.7);
    let base_bot = Vec2::new(cx, cy + size * 0.7);
    DrawElement {
        layer: 1,
        shader: ShaderKind::Sprite,
        effect: DrawEffect::NONE,
        texture: tex,
        uv: UvRect::FULL,
        tint,
        positions: vec![tip, base_top, base_bot],
        uvs: vec![Vec2::ZERO, Vec2::ZERO, Vec2::ZERO],
    }
}

fn quad_uvs() -> Vec<Vec2> {
    vec![
        Vec2::ZERO,
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::ZERO,
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ]
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("adventure-engine — 04-walking")
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
        let tex = renderer
            .upload_texture(1, 1, &[255u8, 255, 255, 255])
            .expect("texture");
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
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let (Some(world), Some(window)) = (self.world.as_mut(), self.window.as_ref()) {
                    // We don't have direct access to cursor pos here; use a fallback
                    // resource stored from the last CursorMoved.
                    let size = window.inner_size();
                    // Use the centre if we haven't seen a CursorMoved yet.
                    let pos = self
                        .last_click_pixel
                        .unwrap_or_else(|| Vec2::new(size.width as f32 * 0.5, size.height as f32 * 0.5));
                    let normalized = Vec2::new(pos.x / size.width as f32, pos.y / size.height as f32);
                    let mut pending = world.resource_mut::<PendingClick>();
                    pending.target = Some(normalized);
                    tracing::info!("click at {:?} (normalized {:?})", pos, normalized);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_click_pixel = Some(Vec2::new(position.x as f32, position.y as f32));
            }
            WindowEvent::RedrawRequested => {
                // Tick world.
                let now = Instant::now();
                let dt = now - self.last_frame;
                self.last_frame = now;
                if let (Some(world), Some(window)) =
                    (self.world.as_mut(), self.window.as_ref())
                {
                    let size = window.inner_size();
                    self.schedule.run(
                        world,
                        dt,
                        size.width as f32,
                        size.height as f32,
                    );
                }
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
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_max_level(tracing::Level::INFO)
        .init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    let _ = (SmolStr::new_inline(""), Duration::from_secs(0));
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
