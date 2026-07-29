//! Shawshank Cell Block C — MVP PAC spine on adventure-engine.
//!
//! ```text
//! cargo test -p example-08-shawshank-pac
//! cargo run -p example-08-shawshank-pac -- --headless
//! cargo run -p example-08-shawshank-pac
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use adventure_core::math::Vec2;
use adventure_dialogue::{DialogRunner, DialogTree};
use adventure_render2d::{DrawElement, ElementBatcher, TextureId, WgpuRenderer};
use adventure_scene::hotspot::{HotspotKind, OnClick};
use adventure_scene::scene::Scene;
use adventure_scripting::ScriptHost;
use adventure_state::{Tag, Tags, VarTable};
use adventure_ui::{DialogBox, DialogBoxConfig, UiInput};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

fn repo_file(rel: &str) -> PathBuf {
    let candidates = [
        PathBuf::from(rel),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("missing {rel} — run from adventure-engine repo root"))
}

fn load_scene() -> Scene {
    let path = repo_file("assets/scenes/cellblock_c.scene.ron");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    Scene::from_ron(&src).unwrap_or_else(|e| panic!("parse cellblock scene: {e}"))
}

fn load_dialog() -> DialogTree {
    let path = repo_file("assets/dialogs/dialogue_andy_first_meeting.dialog.ron");
    let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let tree = DialogTree::from_ron(&src).unwrap_or_else(|e| panic!("parse dialog: {e}"));
    tree.validate().unwrap_or_else(|e| panic!("validate dialog: {e}"));
    tree
}

struct FlagHost {
    tags: Tags,
    vars: VarTable,
    host: ScriptHost,
    status: String,
    dialog: DialogTree,
    runner: DialogRunner,
    in_dialog: bool,
}

impl FlagHost {
    fn new(dialog: DialogTree) -> Self {
        let runner = DialogRunner::new(&dialog);
        Self {
            tags: Tags::new(),
            vars: VarTable::new(),
            host: ScriptHost::new(),
            status: "Cell Block C — 1 look Red · N next day · 2 look Andy · 3 talk · 4 stone".into(),
            dialog,
            runner,
            in_dialog: false,
        }
    }

    fn has(&self, path: &str) -> bool {
        Tag::new(path).map(|t| self.tags.has(&t)).unwrap_or(false)
    }

    fn add(&mut self, path: &str) {
        if let Ok(t) = Tag::new(path) {
            self.tags.add(t);
        }
    }

    fn examined_cell(&self) -> bool { self.has("State.Flag.ExaminedCell") }
    fn andy_arrived(&self) -> bool { self.has("State.Flag.AndyArrived") }
    fn noticed_andy(&self) -> bool { self.has("State.Flag.NoticedAndy") }
    fn can_talk_andy(&self) -> bool { self.has("State.Flag.CanTalkAndy") || self.noticed_andy() }
    fn met_andy(&self) -> bool { self.has("State.Flag.MetAndy") }
    fn knows_red(&self) -> bool { self.has("State.Flag.KnowsRedBusiness") }

    fn next_day(&mut self) {
        if !self.examined_cell() {
            self.status = "Examine Red's cell first (1).".into();
            return;
        }
        self.add("State.Flag.AndyArrived");
        self.status = "Next day — Andy's cell occupied. Look (2), then Talk (3).".into();
    }

    fn examine_red_cell(&mut self) {
        self.add("State.Flag.ExaminedCell");
        self.status = "Home sweet home. Twenty years I've called this eight-by-eight my castle.".into();
    }

    fn examine_andy_cell(&mut self) {
        if !self.andy_arrived() {
            self.status = "Empty cell. Won't be for long. Bus comes tomorrow.".into();
            return;
        }
        self.add("State.Flag.NoticedAndy");
        self.add("State.Flag.CanTalkAndy");
        self.status = "The new fish. Andy Dufresne. Something different about that one.".into();
    }

    fn talk_andy(&mut self) {
        if !self.andy_arrived() {
            self.status = "Nobody there yet.".into();
            return;
        }
        if !self.can_talk_andy() {
            self.status = "Look at the new fish first.".into();
            return;
        }
        if self.met_andy() {
            self.status = "You've already spoken with Andy.".into();
            return;
        }
        match self.runner.start(&self.dialog, &self.host, &mut self.vars, &mut self.tags) {
            Ok(()) => {
                self.in_dialog = true;
                self.status = "Talking to Andy…".into();
            }
            Err(e) => self.status = format!("dialog start failed: {e}"),
        }
    }

    fn use_loose_stone(&mut self) {
        if !self.knows_red() {
            self.status = "Nothing there… yet.".into();
            return;
        }
        self.status = "My little insurance policy. Whatever you need, Red can get it.".into();
    }

    fn dispatch_action(&mut self, action: &str) {
        match action {
            "examine_red_cell" => self.examine_red_cell(),
            "examine_andy_cell" => self.examine_andy_cell(),
            "talk_andy" => self.talk_andy(),
            "use_loose_stone" => self.use_loose_stone(),
            "next_day" => self.next_day(),
            other => self.status = format!("unknown action: {other}"),
        }
    }

    fn finish_dialog_if_done(&mut self) {
        if self.in_dialog && self.runner.is_finished() {
            self.in_dialog = false;
            if !self.met_andy() { self.add("State.Flag.MetAndy"); }
            if !self.knows_red() { self.add("State.Flag.KnowsRedBusiness"); }
            self.status = "Conversation ended. Loose stone may be usable (4).".into();
        }
    }
}

fn walkthrough() -> Result<(), String> {
    let scene = load_scene();
    let room = scene.entry().ok_or("missing entry room")?;
    for id in ["hs_red_cell", "hs_andy_cell", "hs_andy_cell_talk", "hs_contraband_spot"] {
        room.hotspot(id).ok_or_else(|| format!("missing hotspot {id}"))?;
    }
    if room.hotspot("hs_andy_cell_talk").unwrap().kind != HotspotKind::Talk {
        return Err("talk hotspot kind".into());
    }

    let mut host = FlagHost::new(load_dialog());
    host.dispatch_action("examine_red_cell");
    if !host.examined_cell() { return Err("examined_cell".into()); }
    host.dispatch_action("next_day");
    if !host.andy_arrived() { return Err("andy_arrived".into()); }
    host.dispatch_action("examine_andy_cell");
    if !host.noticed_andy() { return Err("noticed_andy".into()); }
    host.dispatch_action("talk_andy");
    if !host.in_dialog { return Err("dialog not started".into()); }

    let mut steps = 0;
    while !host.runner.is_finished() {
        steps += 1;
        if steps > 40 { return Err("dialog runaway".into()); }
        let visible = host.runner.visible_choices(&host.dialog, &host.host, &host.vars, &host.tags);
        if !visible.is_empty() {
            let src = visible[0].source_index;
            host.runner.choose(&host.dialog, &host.host, &mut host.vars, &mut host.tags, src)
                .map_err(|e| e.to_string())?;
        } else {
            host.runner.advance(&host.dialog, &host.host, &mut host.vars, &mut host.tags)
                .map_err(|e| e.to_string())?;
        }
    }
    host.finish_dialog_if_done();
    if !host.met_andy() { return Err("met_andy missing".into()); }
    if !host.knows_red() { return Err("knows_red missing".into()); }
    host.dispatch_action("use_loose_stone");
    if !host.status.contains("insurance") {
        return Err(format!("loose stone: {}", host.status));
    }
    Ok(())
}

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    texture: Option<TextureId>,
    scene: Scene,
    host: FlagHost,
    dbox: DialogBox,
    last_frame: Instant,
    cursor_pos: Vec2,
    click_this_frame: Option<Vec2>,
    should_close: bool,
}

impl App {
    fn new() -> Self {
        let dialog = load_dialog();
        Self {
            window: None,
            instance: wgpu::Instance::default(),
            surface: None,
            adapter: None,
            renderer: None,
            texture: None,
            scene: load_scene(),
            host: FlagHost::new(dialog),
            dbox: DialogBox::new(DialogBoxConfig::default()),
            last_frame: Instant::now(),
            cursor_pos: Vec2::new(400.0, 300.0),
            click_this_frame: None,
            should_close: false,
        }
    }

    fn handle_key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::Escape) => self.should_close = true,
            Key::Character(c) if c.as_str() == "1" && !self.host.in_dialog => {
                self.host.dispatch_action("examine_red_cell");
            }
            Key::Character(c) if c.as_str() == "2" && !self.host.in_dialog => {
                self.host.dispatch_action("examine_andy_cell");
            }
            Key::Character(c) if c.as_str() == "3" && !self.host.in_dialog => {
                self.host.dispatch_action("talk_andy");
            }
            Key::Character(c) if c.as_str() == "4" && !self.host.in_dialog => {
                self.host.dispatch_action("use_loose_stone");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("n") && !self.host.in_dialog => {
                self.host.dispatch_action("next_day");
            }
            _ => {}
        }
        tracing::info!("{}", self.host.status);
    }

    fn click_world(&mut self) {
        if self.host.in_dialog { return; }
        let Some(room) = self.scene.entry() else { return; };
        let Some(window) = &self.window else { return; };
        let size = window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        if w <= 0.0 || h <= 0.0 { return; }
        let p = Vec2::new(self.cursor_pos.x / w, self.cursor_pos.y / h);
        for hs in room.hotspots.iter().rev() {
            if hs.id.as_str() == "hs_andy_cell_talk"
                && (!self.host.andy_arrived() || !self.host.can_talk_andy())
            {
                continue;
            }
            if hs.id.as_str() == "hs_contraband_spot" && !self.host.knows_red() {
                continue;
            }
            if hs.contains(p) {
                if let OnClick::Action(a) = &hs.on_click {
                    self.host.dispatch_action(a.as_str());
                    tracing::info!("{}", self.host.status);
                }
                break;
            }
        }
    }

    fn render(&mut self) {
        let (Some(surface), Some(_adapter), Some(r), Some(_window), Some(tex)) = (
            self.surface.as_ref(),
            self.adapter.as_ref(),
            self.renderer.as_mut(),
            self.window.as_ref(),
            self.texture,
        ) else {
            return;
        };
        let size = _window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        if w == 0.0 || h == 0.0 { return; }
        let view_proj = WgpuRenderer::ortho(w, h);
        let input = UiInput::new(self.cursor_pos, self.click_this_frame);

        let mut ui_elements: Vec<DrawElement> = Vec::new();
        let (advance, pick_src) = if self.host.in_dialog {
            let out = self.dbox.draw(
                &mut ui_elements,
                &input,
                &self.host.runner,
                &self.host.dialog,
                &self.host.host,
                &self.host.vars,
                &self.host.tags,
            );
            let pick = out.picked_visible_index.and_then(|i| {
                out.visible_choices.get(i).map(|c| c.source_index)
            });
            (out.advance_requested, pick)
        } else {
            (false, None)
        };

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
            wgpu::Color { r: 0.07, g: 0.08, b: 0.10, a: 1.0 },
        );

        if advance {
            let _ = self.host.runner.advance(
                &self.host.dialog, &self.host.host, &mut self.host.vars, &mut self.host.tags,
            );
        }
        if let Some(src) = pick_src {
            let _ = self.host.runner.choose(
                &self.host.dialog, &self.host.host, &mut self.host.vars, &mut self.host.tags, src,
            );
        }
        self.host.finish_dialog_if_done();
        self.click_this_frame = None;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let attrs = Window::default_attributes()
            .with_title("Shawshank PAC — Cell Block C (example-08)")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720));
        let Ok(window) = event_loop.create_window(attrs) else { return; };
        let window = Arc::new(window);
        let surface = self.instance.create_surface(Arc::clone(&window)).expect("surface");
        let adapter = pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).expect("adapter");
        let mut renderer = pollster::block_on(WgpuRenderer::new_for_surface(&surface, &self.instance))
            .expect("renderer");
        let tex = renderer.upload_texture(1, 1, &[255u8, 255, 255, 255]).expect("texture");
        let size = window.inner_size();
        renderer.configure_surface(&surface, &adapter, (size.width, size.height));
        self.window = Some(window);
        self.surface = Some(surface);
        self.adapter = Some(adapter);
        self.renderer = Some(renderer);
        self.texture = Some(tex);
        tracing::info!("{}", self.host.status);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.should_close = true,
            WindowEvent::RedrawRequested => {
                self.render();
                if self.should_close { event_loop.exit(); }
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
                self.click_world();
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                ..
            } => self.handle_key(&logical_key),
            WindowEvent::Resized(size) => {
                if let (Some(r), Some(surface), Some(adapter)) = (
                    self.renderer.as_mut(), self.surface.as_ref(), self.adapter.as_ref(),
                ) {
                    r.configure_surface(surface, adapter, (size.width, size.height));
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window { w.request_redraw(); }
        self.last_frame = Instant::now();
    }
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    match walkthrough() {
        Ok(()) => tracing::info!("walkthrough OK"),
        Err(e) => {
            eprintln!("walkthrough FAILED: {e}");
            std::process::exit(1);
        }
    }
    if std::env::args().any(|a| a == "--headless") { return; }
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cellblock_walkthrough() {
        walkthrough().expect("shawshank MVP walkthrough");
    }

    #[test]
    fn fixtures_load() {
        let s = load_scene();
        assert_eq!(s.entry_room.as_str(), "cellblock_c");
        assert_eq!(s.name.as_str(), "shawshank_cellblock");
        let t = load_dialog();
        assert_eq!(t.id.as_str(), "dialogue_andy_first_meeting");
        t.validate().unwrap();
    }
}
