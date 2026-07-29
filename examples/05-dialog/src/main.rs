//! Phase 5 — Example 05: Branching dialog with Rhai conditions + side effects.
//!
//! Loads `assets/dialogs/bob_intro.dialog.ron` and drives a dialog panel
//! with choice buttons. Click a choice (or the panel on linear nodes) to
//! advance. No world rendering — keeps the demo focused on dialogue +
//! scripting + UI.
//!
//! The tree:
//!   ```text
//!   intro → ask (branching)
//!            ├─ "Tell me a secret"  [gated by has_tag("State.NPC.Bob.Met")]
//!            │     → secret → ask (loops back)
//!            ├─ "Say hello back"    [side effect: add_tag + set_int]
//!            │     → ask (loops back, secret now visible)
//!            └─ "Goodbye"            → (terminal)
//!   ```
//!
//! On first visit only "Say hello back" + "Goodbye" are visible. After
//! "Say hello back", the tag is set and "Tell me a secret" appears.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use adventure_core::math::Vec2;
use adventure_dialogue::{DialogRunner, DialogTree};
use adventure_render2d::{DrawElement, ElementBatcher, TextureId, WgpuRenderer};
use adventure_scripting::ScriptHost;
use adventure_state::{Tag, Tags, VarTable};
use adventure_ui::{DialogBox, DialogBoxConfig, UiInput};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Load the Phase 5 fixture (workspace-relative path).
fn load_tree() -> DialogTree {
    let candidates = [
        PathBuf::from("assets/dialogs/bob_intro.dialog.ron"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/dialogs/bob_intro.dialog.ron"),
    ];
    let path = candidates
        .into_iter()
        .find(|p| p.is_file())
        .expect("assets/dialogs/bob_intro.dialog.ron not found — run from repo root or workspace");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let tree = DialogTree::from_ron(&src).expect("parse bob_intro.dialog.ron");
    tree.validate().expect("validate bob_intro.dialog.ron");
    tree
}

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    texture: Option<TextureId>,
    tree: DialogTree,
    runner: DialogRunner,
    host: ScriptHost,
    vars: VarTable,
    tags: Tags,
    dbox: DialogBox,
    last_frame: Instant,
    cursor_pos: Vec2,
    click_this_frame: Option<Vec2>,
    should_close: bool,
    /// Log only when the current node id changes.
    last_logged_node: Option<String>,
}

impl App {
    fn new() -> Self {
        let tree = load_tree();
        let runner = DialogRunner::new(&tree);
        Self {
            window: None,
            instance: wgpu::Instance::default(),
            surface: None,
            adapter: None,
            renderer: None,
            texture: None,
            tree,
            runner,
            host: ScriptHost::new(),
            vars: VarTable::new(),
            tags: Tags::new(),
            dbox: DialogBox::new(DialogBoxConfig::default()),
            last_frame: Instant::now(),
            cursor_pos: Vec2::new(400.0, 300.0),
            click_this_frame: None,
            should_close: false,
            last_logged_node: None,
        }
    }

    fn start_dialog(&mut self) {
        if let Err(e) = self
            .runner
            .start(&self.tree, &self.host, &mut self.vars, &mut self.tags)
        {
            tracing::error!("dialog start failed: {e}");
        }
        self.last_logged_node = None;
    }

    fn log_node_if_changed(&mut self, speaker: Option<&str>, line: Option<&str>) {
        let id = self.runner.current_id().unwrap_or("<finished>").to_string();
        if self.last_logged_node.as_deref() == Some(id.as_str()) {
            return;
        }
        self.last_logged_node = Some(id.clone());
        if self.runner.is_finished() {
            tracing::info!("conversation finished");
            return;
        }
        tracing::info!(
            node = %id,
            speaker = speaker.unwrap_or(""),
            "dialog"
        );
        if let Some(line) = line {
            tracing::info!("  {line}");
        }
        let visible = self
            .runner
            .visible_choices(&self.tree, &self.host, &self.vars, &self.tags);
        for (i, c) in visible.iter().enumerate() {
            tracing::info!("    [{}] {}", i + 1, c.text);
        }
        if self.tags.has(&Tag::new("State.NPC.Bob.Met").unwrap()) {
            tracing::info!("  [tag] State.NPC.Bob.Met");
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

        let input = UiInput::new(self.cursor_pos, self.click_this_frame);

        let mut ui_elements: Vec<DrawElement> = Vec::new();
        let out = self.dbox.draw(
            &mut ui_elements,
            &input,
            &self.runner,
            &self.tree,
            &self.host,
            &self.vars,
            &self.tags,
        );

        // Snapshot strings so we can drop `out` / free borrows before mut self.
        let speaker = out.speaker.clone();
        let line = out.line.clone();
        let advance = out.advance_requested;
        let pick = out.picked_visible_index.and_then(|i| {
            out.visible_choices
                .get(i)
                .map(|c| (c.source_index, c.text.clone()))
        });

        let mut batcher = ElementBatcher::new();
        for el in ui_elements {
            let mut el = el;
            el.texture = tex;
            batcher.push(el);
        }
        let batches = batcher.finish();
        let _ = r.render_frame(
            surface,
            view_proj,
            &batches,
            wgpu::Color {
                r: 0.06,
                g: 0.07,
                b: 0.10,
                a: 1.0,
            },
        );

        // Logging + dialog advance after GPU work (avoids &mut self vs renderer borrow).
        self.log_node_if_changed(speaker.as_deref(), line.as_deref());

        if advance {
            if let Err(e) = self
                .runner
                .advance(&self.tree, &self.host, &mut self.vars, &mut self.tags)
            {
                tracing::warn!("advance: {e}");
            }
        }
        if let Some((source, text)) = pick {
            tracing::info!("→ picked: {text} (source idx {source})");
            if let Err(e) = self.runner.choose(
                &self.tree,
                &self.host,
                &mut self.vars,
                &mut self.tags,
                source,
            ) {
                tracing::warn!("choose: {e}");
            }
        }

        self.click_this_frame = None;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("adventure-engine — 05-dialog")
            .with_inner_size(winit::dpi::PhysicalSize::new(800, 600));
        let Ok(window) = event_loop.create_window(attrs) else {
            return;
        };
        let window = Arc::new(window);
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .unwrap();
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

        self.start_dialog();
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
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.click_this_frame = Some(self.cursor_pos);
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let _dt = now - self.last_frame;
                self.last_frame = now;
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
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("event loop error: {e}");
    }
}
