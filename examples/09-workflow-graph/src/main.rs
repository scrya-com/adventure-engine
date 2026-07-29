//! Example 09 — **Rhai workflow interpreter + visualizer**
//!
//! Static-parse Grok Build `.rhai` workflows (`meta` / `phase` / `agent` /
//! `parallel`) and either:
//!
//! * print **Mermaid** / JSON to stdout (CI + README), or
//! * open a **wgpu** window drawing the layered DAG (game-engine chrome).
//!
//! ```text
//! # Mermaid for docs
//! cargo run -p example-09-workflow-graph -- \
//!   ../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai
//!
//! # JSON
//! cargo run -p example-09-workflow-graph -- --json path/to/workflow.rhai
//!
//! # Live DAG window
//! cargo run -p example-09-workflow-graph -- --window path/to/workflow.rhai
//! ```
//!
//! Not a Rhai runtime — structural graph only (see `adventure-workflow-graph`).

use std::path::PathBuf;
use std::sync::Arc;

use adventure_core::math::Vec2;
use adventure_render2d::{
    DrawEffect, DrawElement, ElementBatcher, ShaderKind, TextureId, Tint, UvRect, WgpuRenderer,
};
use adventure_workflow_graph::{
    kind_border, kind_color, parse_workflow_file, GraphLayout, NodeKind, WorkflowGraph,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut windowed = false;
    let mut json = false;
    let mut out_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--window" | "-w" => {
                windowed = true;
                args.remove(i);
            }
            "--json" | "-j" => {
                json = true;
                args.remove(i);
            }
            "--out" | "-o" => {
                args.remove(i);
                if i < args.len() {
                    out_path = Some(PathBuf::from(&args[i]));
                    args.remove(i);
                }
            }
            "-h" | "--help" => {
                print_help();
                return;
            }
            _ => i += 1,
        }
    }

    let path = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(default_workflow_path);

    if !path.is_file() {
        eprintln!("workflow not found: {}", path.display());
        eprintln!("pass a path to a .rhai workflow (see --help)");
        std::process::exit(1);
    }

    let graph = parse_workflow_file(&path).unwrap_or_else(|e| {
        eprintln!("parse error: {e}");
        std::process::exit(1);
    });

    eprintln!("{}", graph.summary_line());
    if let Some(ref p) = graph.source_path {
        eprintln!("source: {p}");
    }

    if windowed {
        run_window(graph);
        return;
    }

    let body = if json {
        graph.to_json_pretty().expect("json")
    } else {
        let mut md = format!(
            "# {}\n\n{}\n\n",
            graph.name,
            graph.description
        );
        if let Some(ref w) = graph.when_to_use {
            md.push_str(&format!("**When:** {w}\n\n"));
        }
        md.push_str("## Stats\n\n");
        md.push_str(&format!(
            "| metric | count |\n| --- | --- |\n| phases | {} |\n| phase() | {} |\n| agent() | {} |\n| parallel() | {} |\n| complete() | {} |\n| gates | {} |\n\n",
            graph.phases.len(),
            graph.stats.phase_calls,
            graph.stats.agent_calls,
            graph.stats.parallel_calls,
            graph.stats.complete_calls,
            graph.stats.gate_calls,
        ));
        md.push_str("## Graph\n\n");
        md.push_str(&graph.to_mermaid());
        md
    };

    if let Some(out) = out_path {
        std::fs::write(&out, &body).expect("write --out");
        eprintln!("wrote {}", out.display());
    } else {
        print!("{body}");
    }
}

fn print_help() {
    eprintln!(
        "example-09-workflow-graph — static Rhai workflow → Mermaid / JSON / wgpu DAG\n\n\
         USAGE:\n\
           cargo run -p example-09-workflow-graph -- [FLAGS] <workflow.rhai>\n\n\
         FLAGS:\n\
           --window, -w     open wgpu DAG viewer\n\
           --json, -j       emit JSON instead of Mermaid markdown\n\
           --out, -o PATH   write output to PATH\n\
           -h, --help       this help\n"
    );
}

fn default_workflow_path() -> PathBuf {
    let candidates = [
        PathBuf::from("../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"),
        PathBuf::from("../../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"),
        PathBuf::from("/home/johndpope/Documents/GitHub/PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("workflow.rhai"))
}

// ── wgpu viewer ──────────────────────────────────────────────────────────

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    white: Option<TextureId>,
    graph: WorkflowGraph,
    layout: GraphLayout,
    pan: Vec2,
    zoom: f32,
    cursor: Vec2,
    should_close: bool,
    dragging: bool,
}

impl App {
    fn new(graph: WorkflowGraph) -> Self {
        let layout = graph.layout_layers(160.0, 52.0, 40.0, 20.0);
        Self {
            window: None,
            instance: wgpu::Instance::default(),
            surface: None,
            adapter: None,
            renderer: None,
            white: None,
            graph,
            layout,
            pan: Vec2::new(40.0, 60.0),
            zoom: 1.0,
            cursor: Vec2::ZERO,
            should_close: false,
            dragging: false,
        }
    }

    fn solid(
        white: TextureId,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        layer: i32,
        tint: Tint,
    ) -> DrawElement {
        DrawElement {
            layer,
            shader: ShaderKind::Sprite,
            effect: DrawEffect::NONE,
            texture: white,
            uv: UvRect::FULL,
            tint,
            positions: vec![
                Vec2::new(x0, y0),
                Vec2::new(x1, y0),
                Vec2::new(x1, y1),
                Vec2::new(x0, y0),
                Vec2::new(x1, y1),
                Vec2::new(x0, y1),
            ],
            uvs: vec![
                Vec2::ZERO,
                Vec2::new(1.0, 0.0),
                Vec2::new(1.0, 1.0),
                Vec2::ZERO,
                Vec2::new(1.0, 1.0),
                Vec2::new(0.0, 1.0),
            ],
        }
    }

    fn render(&mut self) {
        let (Some(window), Some(surface), Some(r), Some(white)) = (
            self.window.as_ref(),
            self.surface.as_ref(),
            self.renderer.as_mut(),
            self.white,
        ) else {
            return;
        };
        let size = window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        if w < 1.0 || h < 1.0 {
            return;
        }
        let view_proj = WgpuRenderer::ortho(w, h);
        let mut batcher = ElementBatcher::new();

        // Background
        batcher.push(Self::solid(
            white,
            0.0,
            0.0,
            w,
            h,
            0,
            Tint::rgba(0.04, 0.05, 0.07, 1.0),
        ));

        // Title bar
        batcher.push(Self::solid(
            white,
            0.0,
            0.0,
            w,
            44.0,
            1,
            Tint::rgba(0.08, 0.10, 0.13, 1.0),
        ));
        batcher.push(Self::solid(
            white,
            0.0,
            42.0,
            w,
            44.0,
            2,
            Tint::rgba(0.77, 0.65, 0.45, 0.7),
        ));

        let z = self.zoom;
        let pan = self.pan;

        // Edges first
        for e in &self.layout.edges {
            let Some(a) = self.layout.nodes.iter().find(|n| n.id == e.from) else {
                continue;
            };
            let Some(b) = self.layout.nodes.iter().find(|n| n.id == e.to) else {
                continue;
            };
            let x0 = pan.x + (a.x + a.w) * z;
            let y0 = pan.y + (a.y + a.h * 0.5) * z;
            let x1 = pan.x + b.x * z;
            let y1 = pan.y + (b.y + b.h * 0.5) * z;
            // thick line as thin rect along segment (axis-ish approximation)
            let mx = (x0 + x1) * 0.5;
            let my = (y0 + y1) * 0.5;
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            // horizontal then vertical elbow
            batcher.push(Self::solid(
                white,
                x0.min(mx),
                y0 - 1.5,
                x0.max(mx),
                y0 + 1.5,
                3,
                Tint::rgba(0.45, 0.50, 0.55, 0.7),
            ));
            batcher.push(Self::solid(
                white,
                mx - 1.5,
                y0.min(y1),
                mx + 1.5,
                y0.max(y1),
                3,
                Tint::rgba(0.45, 0.50, 0.55, 0.7),
            ));
            batcher.push(Self::solid(
                white,
                mx.min(x1),
                y1 - 1.5,
                mx.max(x1),
                y1 + 1.5,
                3,
                Tint::rgba(0.45, 0.50, 0.55, 0.7),
            ));
            let _ = (my, len);
        }

        // Nodes
        for n in &self.layout.nodes {
            let x0 = pan.x + n.x * z;
            let y0 = pan.y + n.y * z;
            let x1 = x0 + n.w * z;
            let y1 = y0 + n.h * z;
            let (r, g, b, a) = kind_color(n.kind);
            let (br, bg, bb, ba) = kind_border(n.kind);
            batcher.push(Self::solid(
                white,
                x0,
                y0,
                x1,
                y1,
                4,
                Tint::rgba(r, g, b, a),
            ));
            // border
            let t = 2.0;
            batcher.push(Self::solid(
                white,
                x0,
                y0,
                x1,
                y0 + t,
                5,
                Tint::rgba(br, bg, bb, ba),
            ));
            batcher.push(Self::solid(
                white,
                x0,
                y1 - t,
                x1,
                y1,
                5,
                Tint::rgba(br, bg, bb, ba),
            ));
            batcher.push(Self::solid(
                white,
                x0,
                y0,
                x0 + t,
                y1,
                5,
                Tint::rgba(br, bg, bb, ba),
            ));
            batcher.push(Self::solid(
                white,
                x1 - t,
                y0,
                x1,
                y1,
                5,
                Tint::rgba(br, bg, bb, ba),
            ));
            // kind color bar on left
            let accent = match n.kind {
                NodeKind::Parallel => Tint::rgba(0.36, 0.62, 0.83, 1.0),
                NodeKind::Agent => Tint::rgba(0.24, 0.60, 0.42, 1.0),
                NodeKind::Phase => Tint::rgba(0.77, 0.65, 0.45, 1.0),
                NodeKind::Gate => Tint::rgba(0.77, 0.36, 0.36, 1.0),
                _ => Tint::rgba(0.5, 0.55, 0.6, 1.0),
            };
            batcher.push(Self::solid(white, x0, y0, x0 + 6.0 * z, y1, 6, accent));
        }

        // Legend strip bottom
        batcher.push(Self::solid(
            white,
            0.0,
            h - 36.0,
            w,
            h,
            8,
            Tint::rgba(0.06, 0.07, 0.09, 0.95),
        ));

        let batches = batcher.finish();
        let _ = r.render_frame(
            surface,
            view_proj,
            &batches,
            wgpu::Color {
                r: 0.04,
                g: 0.05,
                b: 0.07,
                a: 1.0,
            },
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let title = format!(
            "Workflow graph — {} ({})",
            self.graph.name,
            self.graph.summary_line()
        );
        let attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 800));
        let Ok(window) = event_loop.create_window(attrs) else {
            return;
        };
        let window = Arc::new(window);
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .expect("surface");
        let adapter = pollster::block_on(self.instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ))
        .expect("adapter");
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_for_surface(&surface, &self.instance))
                .expect("renderer");
        let white = renderer
            .upload_texture(1, 1, &[255u8, 255, 255, 255])
            .expect("white");
        let size = window.inner_size();
        renderer.configure_surface(&surface, &adapter, (size.width, size.height));
        self.window = Some(window);
        self.surface = Some(surface);
        self.adapter = Some(adapter);
        self.renderer = Some(renderer);
        self.white = Some(white);
        tracing::info!("{}", self.graph.summary_line());
        tracing::info!("drag to pan · scroll not wired · Esc quit · M prints mermaid to log");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.should_close = true,
            WindowEvent::RedrawRequested => {
                self.render();
                if self.should_close {
                    event_loop.exit();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Vec2::new(position.x as f32, position.y as f32);
                if self.dragging {
                    self.pan.x += p.x - self.cursor.x;
                    self.pan.y += p.y - self.cursor.y;
                }
                self.cursor = p;
            }
            WindowEvent::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                self.dragging = state == ElementState::Pressed;
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match logical_key {
                Key::Named(NamedKey::Escape) => self.should_close = true,
                Key::Character(c) if c.eq_ignore_ascii_case("m") => {
                    tracing::info!("\n{}", self.graph.to_mermaid());
                }
                Key::Character(c) if c.eq_ignore_ascii_case("=") || c.as_str() == "+" => {
                    self.zoom = (self.zoom * 1.1).min(3.0);
                }
                Key::Character(c) if c.as_str() == "-" => {
                    self.zoom = (self.zoom / 1.1).max(0.4);
                }
                _ => {}
            },
            WindowEvent::Resized(size) => {
                if let (Some(r), Some(surface), Some(adapter)) = (
                    self.renderer.as_mut(),
                    self.surface.as_ref(),
                    self.adapter.as_ref(),
                ) {
                    r.configure_surface(surface, adapter, (size.width, size.height));
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

fn run_window(graph: WorkflowGraph) {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(graph);
    let _ = event_loop.run_app(&mut app);
}
