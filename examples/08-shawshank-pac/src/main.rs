//! Shawshank Cell Block C — playable chrome + PAC spine on adventure-engine.
//!
//! Data:
//!   * `assets/scenes/cellblock_c.scene.ron`  (from ncp room.json hotspots)
//!   * `assets/dialogs/dialogue_andy_first_meeting.dialog.ron`
//!   * `examples/06-shawshank-pac/assets/` — cellblock_bg + hub portraits
//!
//! Host flag machine mirrors `ncp/demos/shawshank-pac/game.js`:
//!   examine Red → Next day → examine Andy → talk → loose stone
//!
//! Windowed host:
//!   * full-bleed bg + portraits when flags flip
//!   * **verb bar** (LOOK / TALK / USE) + **NEXT DAY** button
//!   * bright hotspots + hover labels + always-on status strip
//!   * dialog panel with bitmap-text speaker/line
//!
//! ```text
//! cargo test -p example-08-shawshank-pac
//! cargo run -p example-08-shawshank-pac -- --headless
//! cargo run -p example-08-shawshank-pac -- --playtest /tmp/score.json
//! cargo run -p example-08-shawshank-pac   # windowed
//! ```

mod bitmap_text;
mod playtest;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use adventure_core::math::Vec2;
use adventure_dialogue::{DialogRunner, DialogTree};
use adventure_render2d::{
    DrawEffect, DrawElement, ElementBatcher, ShaderKind, TextureId, Tint, UvRect, WgpuRenderer,
};
use adventure_scene::hotspot::{HotspotKind, OnClick};
use adventure_scene::scene::Scene;
use adventure_scripting::ScriptHost;
use adventure_state::flag_paths::shawshank as flags;
use adventure_state::{Tag, Tags, VarTable};
use adventure_ui::layout::Rect;
use adventure_ui::{DialogBox, DialogBoxConfig, UiContext, UiInput, UI_LAYER};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use bitmap_text::{draw_text, text_width};
use playtest::{score_rubric, timed, PlaytestReport, TaskResult};

/// Content-pack stills (shared with the NCP HTML demo / example-06 pack).
const ASSET_BG: &str = "examples/06-shawshank-pac/assets/cellblock_bg.png";
const ASSET_PORT_RED: &str = "examples/06-shawshank-pac/assets/portrait_red.png";
const ASSET_PORT_ANDY: &str = "examples/06-shawshank-pac/assets/portrait_andy.png";

/// Active SCUMM-style verb (click world after selecting).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Look,
    Talk,
    Use,
}

impl Verb {
    fn label(self) -> &'static str {
        match self {
            Verb::Look => "LOOK",
            Verb::Talk => "TALK",
            Verb::Use => "USE",
        }
    }
}

/// Chrome strip heights (px).
const STATUS_H: f32 = 44.0;
const VERB_H: f32 = 56.0;

/// HTML demo portrait layout (`style.css`: width 11%, left/top %).
const PORT_W: f32 = 0.14;
const PORT_RED_X: f32 = 0.16;
const PORT_RED_Y: f32 = 0.26;
const PORT_ANDY_X: f32 = 0.30;
const PORT_ANDY_Y: f32 = 0.26;

fn try_repo_file(rel: &str) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(rel),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn repo_file(rel: &str) -> PathBuf {
    try_repo_file(rel).unwrap_or_else(|| panic!("missing {rel} — run from adventure-engine repo root"))
}

/// Decode a PNG to RGBA8. Returns `None` if missing or unreadable — never panics.
fn load_rgba_png(rel: &str) -> Option<(u32, u32, Vec<u8>)> {
    let path = try_repo_file(rel)?;
    let img = image::open(&path)
        .map_err(|e| tracing::warn!("skip {}: {e}", path.display()))
        .ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Some((w, h, rgba.into_raw()))
}

/// Axis-aligned textured quad in window pixel space (y-down, matching winit).
fn textured_quad(
    texture: TextureId,
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
        texture,
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
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        ],
    }
}

/// Soft-upload a content-pack PNG; `None` if file missing or GPU upload fails.
fn try_upload_png(renderer: &mut WgpuRenderer, rel: &str) -> Option<TextureId> {
    let (w, h, rgba) = load_rgba_png(rel)?;
    match renderer.upload_texture(w, h, &rgba) {
        Ok(id) => {
            tracing::info!("loaded texture {rel} ({w}x{h}) → {:?}", id);
            Some(id)
        }
        Err(e) => {
            tracing::warn!("upload {rel} failed: {e}");
            None
        }
    }
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

// ── Flag host ────────────────────────────────────────────────────────

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

    fn examined_cell(&self) -> bool {
        self.has(flags::EXAMINED_CELL)
    }
    fn andy_arrived(&self) -> bool {
        self.has(flags::ANDY_ARRIVED)
    }
    fn noticed_andy(&self) -> bool {
        self.has(flags::NOTICED_ANDY)
    }
    fn can_talk_andy(&self) -> bool {
        self.has(flags::CAN_TALK_ANDY) || self.noticed_andy()
    }
    fn met_andy(&self) -> bool {
        self.has(flags::MET_ANDY)
    }
    fn knows_red(&self) -> bool {
        self.has(flags::KNOWS_RED_BUSINESS)
    }

    /// HTML parity: talk when arrived+can_talk; contraband only when knows_red.
    fn hotspot_active(&self, id: &str) -> bool {
        match id {
            "hs_red_cell" | "hs_andy_cell" => true,
            "hs_andy_cell_talk" => self.andy_arrived() && self.can_talk_andy(),
            "hs_contraband_spot" => self.knows_red(),
            _ => true,
        }
    }

    /// Snapshot of the six spine flags (order matches [`flags::ALL`]).
    fn six_flags(&self) -> [bool; 6] {
        [
            self.examined_cell(),
            self.andy_arrived(),
            self.noticed_andy(),
            self.can_talk_andy(),
            self.met_andy(),
            self.knows_red(),
        ]
    }

    fn next_day(&mut self) {
        if !self.examined_cell() {
            self.status = "Examine Red's cell first (1).".into();
            return;
        }
        if self.andy_arrived() {
            self.status = "Andy has already arrived.".into();
            return;
        }
        self.add(flags::ANDY_ARRIVED);
        self.status = "Next day — Andy's cell occupied. Look (2), then Talk (3).".into();
    }

    fn examine_red_cell(&mut self) {
        self.add(flags::EXAMINED_CELL);
        self.status =
            "Home sweet home. Twenty years I've called this eight-by-eight my castle.".into();
    }

    fn examine_andy_cell(&mut self) {
        if !self.andy_arrived() {
            self.status = "Empty cell. Won't be for long. Bus comes tomorrow.".into();
            return;
        }
        self.add(flags::NOTICED_ANDY);
        self.add(flags::CAN_TALK_ANDY);
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
        match self
            .runner
            .start(&self.dialog, &self.host, &mut self.vars, &mut self.tags)
        {
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
            if !self.met_andy() {
                self.add(flags::MET_ANDY);
            }
            if !self.knows_red() {
                self.add(flags::KNOWS_RED_BUSINESS);
            }
            self.status = "Conversation ended. Loose stone may be usable (4).".into();
        }
    }
}

// ── Headless walkthrough ─────────────────────────────────────────────

fn drive_dialog_to_end(host: &mut FlagHost) -> Result<(), String> {
    let mut steps = 0;
    while !host.runner.is_finished() {
        steps += 1;
        if steps > 40 {
            return Err("dialog runaway".into());
        }
        let visible = host
            .runner
            .visible_choices(&host.dialog, &host.host, &host.vars, &host.tags);
        if !visible.is_empty() {
            let src = visible[0].source_index;
            host.runner
                .choose(
                    &host.dialog,
                    &host.host,
                    &mut host.vars,
                    &mut host.tags,
                    src,
                )
                .map_err(|e| e.to_string())?;
        } else {
            host.runner
                .advance(&host.dialog, &host.host, &mut host.vars, &mut host.tags)
                .map_err(|e| e.to_string())?;
        }
    }
    host.finish_dialog_if_done();
    Ok(())
}

fn walkthrough() -> Result<(), String> {
    let scene = load_scene();
    let room = scene.entry().ok_or("missing entry room")?;
    for id in [
        "hs_red_cell",
        "hs_andy_cell",
        "hs_andy_cell_talk",
        "hs_contraband_spot",
    ] {
        room.hotspot(id)
            .ok_or_else(|| format!("missing hotspot {id}"))?;
    }
    let talk = room.hotspot("hs_andy_cell_talk").unwrap();
    if talk.kind != HotspotKind::Talk {
        return Err("talk hotspot kind".into());
    }
    let examine = room.hotspot("hs_andy_cell").unwrap();
    if examine.kind != HotspotKind::Examine {
        return Err("andy examine hotspot kind".into());
    }

    let mut host = FlagHost::new(load_dialog());

    // ── Gate: N before Look is rejected with status text ────────────
    host.dispatch_action("next_day");
    if host.andy_arrived() {
        return Err("N before Look must not set andy_arrived".into());
    }
    if !host.status.contains("Examine Red's cell first") {
        return Err(format!("N-before-Look status: {}", host.status));
    }
    if host.hotspot_active("hs_andy_cell_talk") {
        return Err("talk hotspot must stay hidden before spine start".into());
    }
    if host.hotspot_active("hs_contraband_spot") {
        return Err("contraband must stay hidden before knows_red".into());
    }

    // ── 1) Look Red → examined_cell ─────────────────────────────────
    host.dispatch_action("examine_red_cell");
    if !host.examined_cell() {
        return Err("examined_cell".into());
    }
    // Still no talk / contraband
    if host.hotspot_active("hs_andy_cell_talk") {
        return Err("talk hotspot active after only Red look".into());
    }
    if host.hotspot_active("hs_contraband_spot") {
        return Err("contraband active after only Red look".into());
    }

    // ── 2) Next day → andy_arrived ──────────────────────────────────
    host.dispatch_action("next_day");
    if !host.andy_arrived() {
        return Err("andy_arrived".into());
    }
    // Talk still gated on Look (can_talk)
    if host.hotspot_active("hs_andy_cell_talk") {
        return Err("talk hotspot must stay hidden until can_talk".into());
    }
    host.dispatch_action("talk_andy");
    if host.in_dialog {
        return Err("talk must not start before Look on Andy".into());
    }
    if !host.status.contains("Look at the new fish first") {
        return Err(format!("talk-before-look status: {}", host.status));
    }
    // Contraband still hidden
    host.dispatch_action("use_loose_stone");
    if !host.status.contains("Nothing there") {
        return Err(format!("stone before knows_red: {}", host.status));
    }
    if host.hotspot_active("hs_contraband_spot") {
        return Err("contraband must stay hidden until knows_red".into());
    }

    // ── 3) Look Andy → noticed + can_talk; talk hotspot reveals ─────
    host.dispatch_action("examine_andy_cell");
    if !host.noticed_andy() || !host.can_talk_andy() {
        return Err("noticed_andy / can_talk_andy".into());
    }
    if !host.hotspot_active("hs_andy_cell_talk") {
        return Err("talk hotspot must be active after can_talk".into());
    }
    if host.hotspot_active("hs_contraband_spot") {
        return Err("contraband still hidden until dialog exit".into());
    }

    // ── 4) Talk → dialog → met_andy + knows_red ─────────────────────
    host.dispatch_action("talk_andy");
    if !host.in_dialog {
        return Err("dialog not started".into());
    }
    drive_dialog_to_end(&mut host)?;
    if !host.met_andy() {
        return Err("met_andy missing".into());
    }
    if !host.knows_red() {
        return Err("knows_red missing".into());
    }
    if !host.hotspot_active("hs_contraband_spot") {
        return Err("contraband must reveal after knows_red".into());
    }

    // ── All six flags ───────────────────────────────────────────────
    let six = host.six_flags();
    if six != [true; 6] {
        return Err(format!(
            "expected all six flags true, got {:?} for {:?}",
            six,
            flags::ALL
        ));
    }

    // ── 5) Loose stone usable ───────────────────────────────────────
    host.dispatch_action("use_loose_stone");
    if !host.status.contains("insurance") {
        return Err(format!("loose stone: {}", host.status));
    }
    Ok(())
}

// ── Windowed app ─────────────────────────────────────────────────────

struct App {
    window: Option<Arc<Window>>,
    instance: wgpu::Instance,
    surface: Option<wgpu::Surface<'static>>,
    adapter: Option<wgpu::Adapter>,
    renderer: Option<WgpuRenderer>,
    /// 1×1 white texel for tinted UI / hotspot fills.
    white_tex: Option<TextureId>,
    /// Full-bleed cellblock still (optional — missing file is fine).
    bg_tex: Option<TextureId>,
    port_red: Option<TextureId>,
    port_andy: Option<TextureId>,
    scene: Scene,
    host: FlagHost,
    dbox: DialogBox,
    verb: Verb,
    last_frame: Instant,
    cursor_pos: Vec2,
    click_this_frame: Option<Vec2>,
    /// True when chrome UI consumed the click this frame.
    chrome_ate_click: bool,
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
            white_tex: None,
            bg_tex: None,
            port_red: None,
            port_andy: None,
            scene: load_scene(),
            host: FlagHost::new(dialog),
            dbox: DialogBox::new(DialogBoxConfig {
                viewport: Vec2::new(1280.0, 720.0),
                bottom_margin: VERB_H + 12.0,
                min_height: 160.0,
                ..DialogBoxConfig::default()
            }),
            verb: Verb::Look,
            last_frame: Instant::now(),
            cursor_pos: Vec2::new(400.0, 300.0),
            click_this_frame: None,
            chrome_ate_click: false,
            should_close: false,
        }
    }

    fn handle_key(&mut self, key: &Key) {
        match key {
            Key::Named(NamedKey::Escape) => self.should_close = true,
            Key::Character(c) if c.as_str() == "1" && !self.host.in_dialog => {
                self.verb = Verb::Look;
                self.host.dispatch_action("examine_red_cell");
            }
            Key::Character(c) if c.as_str() == "2" && !self.host.in_dialog => {
                self.verb = Verb::Look;
                self.host.dispatch_action("examine_andy_cell");
            }
            Key::Character(c) if c.as_str() == "3" && !self.host.in_dialog => {
                self.verb = Verb::Talk;
                self.host.dispatch_action("talk_andy");
            }
            Key::Character(c) if c.as_str() == "4" && !self.host.in_dialog => {
                self.verb = Verb::Use;
                self.host.dispatch_action("use_loose_stone");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("n") && !self.host.in_dialog => {
                self.host.dispatch_action("next_day");
            }
            Key::Character(c) if c.eq_ignore_ascii_case("l") => self.verb = Verb::Look,
            Key::Character(c) if c.eq_ignore_ascii_case("t") => self.verb = Verb::Talk,
            Key::Character(c) if c.eq_ignore_ascii_case("u") => self.verb = Verb::Use,
            _ => {}
        }
        tracing::info!("[{}] {}", self.verb.label(), self.host.status);
    }

    /// Map (verb, hotspot action id) → host action or failure status.
    fn apply_verb_on_hotspot(&mut self, action_id: &str) {
        match (self.verb, action_id) {
            (Verb::Look, "examine_red_cell") => self.host.dispatch_action("examine_red_cell"),
            (Verb::Look, "examine_andy_cell") | (Verb::Look, "talk_andy") => {
                // Looking at Andy's cell (talk hotspot shares geometry) uses examine.
                self.host.dispatch_action("examine_andy_cell");
            }
            (Verb::Talk, "talk_andy") | (Verb::Talk, "examine_andy_cell") => {
                self.host.dispatch_action("talk_andy");
            }
            (Verb::Use, "use_loose_stone") => self.host.dispatch_action("use_loose_stone"),
            (Verb::Use, "examine_red_cell") => {
                // Use Red's cell → change_room stub message
                self.host.status = "Red's cell door is locked. Not in this demo slice.".into();
            }
            _ => {
                self.host.status = format!(
                    "Can't {} that. Try another verb or hotspot.",
                    self.verb.label().to_ascii_lowercase()
                );
            }
        }
    }

    fn click_world(&mut self) {
        if self.host.in_dialog || self.chrome_ate_click {
            return;
        }
        let Some(room) = self.scene.entry() else {
            return;
        };
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        // Clicks on chrome strips are not world.
        if self.cursor_pos.y < STATUS_H || self.cursor_pos.y > h - VERB_H {
            return;
        }
        let p = Vec2::new(self.cursor_pos.x / w, self.cursor_pos.y / h);
        let mut hit: Option<String> = None;
        for hs in room.hotspots.iter().rev() {
            if !self.host.hotspot_active(hs.id.as_str()) {
                continue;
            }
            if hs.contains(p) {
                if let OnClick::Action(a) = &hs.on_click {
                    hit = Some(a.to_string());
                }
                break;
            }
        }
        if let Some(a) = hit {
            self.apply_verb_on_hotspot(&a);
            tracing::info!("[{}] {}", self.verb.label(), self.host.status);
            return;
        }
        self.host.status = format!(
            "{} — nothing here. Hover a glowing zone, or pick a verb below.",
            self.verb.label()
        );
    }

    fn hotspot_label(id: &str) -> &'static str {
        match id {
            "hs_red_cell" => "RED'S CELL",
            "hs_andy_cell" => "ANDY'S CELL",
            "hs_andy_cell_talk" => "ANDY (TALK)",
            "hs_contraband_spot" => "LOOSE STONE",
            _ => "HOTSPOT",
        }
    }

    /// Verb bar + NEXT DAY — returns true if a button was clicked.
    fn draw_chrome(
        &mut self,
        ui_elements: &mut Vec<DrawElement>,
        input: &UiInput,
        white: TextureId,
        w: f32,
        h: f32,
    ) -> bool {
        let mut ate = false;
        let mut ui = UiContext::new(ui_elements, input);

        // Status panel
        let status_r = Rect::new(Vec2::ZERO, Vec2::new(w, STATUS_H));
        ui.panel(
            status_r,
            Tint::rgba(0.04, 0.05, 0.08, 0.92),
            Tint::rgba(0.85, 0.75, 0.35, 1.0),
            2.0,
            UI_LAYER,
        );

        // Verb strip
        let verb_r = Rect::new(Vec2::new(0.0, h - VERB_H), Vec2::new(w, VERB_H));
        ui.panel(
            verb_r,
            Tint::rgba(0.06, 0.07, 0.1, 0.94),
            Tint::rgba(0.5, 0.55, 0.65, 1.0),
            2.0,
            UI_LAYER,
        );

        let verbs = [Verb::Look, Verb::Talk, Verb::Use];
        let btn_w = 110.0_f32;
        let btn_h = 36.0_f32;
        let gap = 12.0_f32;
        let mut x = 16.0_f32;
        let y = h - VERB_H + (VERB_H - btn_h) * 0.5;
        for v in verbs {
            let r = Rect::new(Vec2::new(x, y), Vec2::new(btn_w, btn_h));
            let st = ui.button(r, UI_LAYER + 2);
            if st.clicked {
                self.verb = v;
                ate = true;
                self.host.status = format!("Verb: {}. Click a glowing hotspot.", v.label());
            }
            // Selected = gold fill overlay
            if self.verb == v {
                ui.rect(r, Tint::rgba(0.75, 0.55, 0.15, 0.45), UI_LAYER + 3);
            }
            x += btn_w + gap;
        }

        // NEXT DAY — large affordance
        let next_w = 150.0_f32;
        let next_r = Rect::new(
            Vec2::new(w - next_w - 16.0, y),
            Vec2::new(next_w, btn_h),
        );
        let next_st = ui.button(next_r, UI_LAYER + 2);
        if next_st.clicked && !self.host.in_dialog {
            self.host.dispatch_action("next_day");
            ate = true;
        }

        // Drop ui borrow before text
        drop(ui);

        // Status text
        let day = if self.host.andy_arrived() {
            "DAY 2"
        } else {
            "DAY 1"
        };
        let line = format!("{}  |  {}", day, self.host.status);
        let trunc: String = line.chars().take(72).collect();
        draw_text(
            ui_elements,
            white,
            12.0,
            12.0,
            &trunc,
            2.0,
            Tint::rgba(0.95, 0.92, 0.8, 1.0),
            UI_LAYER + 5,
        );

        // Verb labels (on top of buttons)
        x = 16.0;
        for v in verbs {
            let label = v.label();
            let tw = text_width(label, 2.0);
            let lx = x + (btn_w - tw) * 0.5;
            let ly = y + 10.0;
            let col = if self.verb == v {
                Tint::rgba(1.0, 0.95, 0.7, 1.0)
            } else {
                Tint::rgba(0.85, 0.88, 0.95, 1.0)
            };
            draw_text(ui_elements, white, lx, ly, label, 2.0, col, UI_LAYER + 5);
            x += btn_w + gap;
        }
        draw_text(
            ui_elements,
            white,
            w - next_w - 8.0,
            y + 10.0,
            "NEXT DAY",
            2.0,
            Tint::rgba(0.7, 0.95, 0.75, 1.0),
            UI_LAYER + 5,
        );

        // Hint under verbs
        draw_text(
            ui_elements,
            white,
            16.0,
            h - 14.0,
            "L/T/U SELECT VERB  ·  CLICK GLOW ZONE  ·  N NEXT DAY  ·  ESC QUIT",
            1.0,
            Tint::rgba(0.55, 0.58, 0.65, 1.0),
            UI_LAYER + 5,
        );

        ate
    }

    fn render(&mut self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Some(white) = self.white_tex else {
            return;
        };
        let size = window.inner_size();
        let (w, h) = (size.width as f32, size.height as f32);
        if w == 0.0 || h == 0.0 {
            return;
        }
        // Keep dialog layout in sync with window (no renderer borrow yet).
        self.dbox = DialogBox::new(DialogBoxConfig {
            viewport: Vec2::new(w, h),
            bottom_margin: VERB_H + 12.0,
            min_height: 160.0,
            ..DialogBoxConfig::default()
        });

        let view_proj = WgpuRenderer::ortho(w, h);
        let input = UiInput::new(self.cursor_pos, self.click_this_frame);

        let mut ui_elements: Vec<DrawElement> = Vec::new();
        self.chrome_ate_click = self.draw_chrome(&mut ui_elements, &input, white, w, h);

        let (advance, pick_src, dlg_speaker, dlg_line, dlg_choices) = if self.host.in_dialog {
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
            let choices: Vec<String> = out
                .visible_choices
                .iter()
                .map(|c| c.text.clone())
                .collect();
            (
                out.advance_requested,
                pick,
                out.speaker.clone(),
                out.line.clone(),
                choices,
            )
        } else {
            (false, None, None, None, Vec::new())
        };

        // Dialog text (engine has no font phase yet — bitmap overlay).
        if let (Some(sp), Some(line)) = (&dlg_speaker, &dlg_line) {
            let body = DialogBox::new(DialogBoxConfig {
                viewport: Vec2::new(w, h),
                bottom_margin: VERB_H + 12.0,
                min_height: 160.0,
                ..DialogBoxConfig::default()
            })
            .body_rect();
            draw_text(
                &mut ui_elements,
                white,
                body.left() + 16.0,
                body.top() + 12.0,
                &sp.to_ascii_uppercase(),
                2.0,
                Tint::rgba(0.95, 0.8, 0.35, 1.0),
                UI_LAYER + 8,
            );
            let line_t: String = line.chars().take(56).collect();
            draw_text(
                &mut ui_elements,
                white,
                body.left() + 16.0,
                body.top() + 36.0,
                &line_t,
                1.5,
                Tint::rgba(0.92, 0.92, 0.95, 1.0),
                UI_LAYER + 8,
            );
            // Numbered choice hints
            for (i, c) in dlg_choices.iter().enumerate() {
                let t: String = format!("{}. {}", i + 1, c.chars().take(40).collect::<String>());
                draw_text(
                    &mut ui_elements,
                    white,
                    body.left() + 20.0,
                    body.bottom() - 28.0 - (dlg_choices.len() - 1 - i) as f32 * 32.0,
                    &t,
                    1.5,
                    Tint::rgba(0.85, 0.9, 1.0, 1.0),
                    UI_LAYER + 8,
                );
            }
        }

        let mut batcher = ElementBatcher::new();

        // Layer 0 — full-bleed cellblock background
        if let Some(bg) = self.bg_tex {
            batcher.push(textured_quad(bg, 0.0, 0.0, w, h, 0, Tint::IDENTITY));
        }

        // Layer 1 — hub portraits (larger, after look / next day)
        let side = PORT_W * w;
        if self.host.examined_cell() {
            if let Some(tex) = self.port_red {
                let x0 = PORT_RED_X * w;
                let y0 = PORT_RED_Y * h;
                batcher.push(textured_quad(
                    tex,
                    x0,
                    y0,
                    x0 + side,
                    y0 + side,
                    1,
                    Tint::IDENTITY,
                ));
            }
        }
        if self.host.andy_arrived() {
            if let Some(tex) = self.port_andy {
                let x0 = PORT_ANDY_X * w;
                let y0 = PORT_ANDY_Y * h;
                batcher.push(textured_quad(
                    tex,
                    x0,
                    y0,
                    x0 + side,
                    y0 + side,
                    1,
                    Tint::IDENTITY,
                ));
            }
        }

        // Layer 2 — bright hotspots + hover label
        let hover_norm = Vec2::new(self.cursor_pos.x / w, self.cursor_pos.y / h);
        if let Some(room) = self.scene.entry() {
            for hs in &room.hotspots {
                if !self.host.hotspot_active(hs.id.as_str()) {
                    continue;
                }
                if hs.polygon.len() < 4 {
                    continue;
                }
                let p0 = hs.polygon[0];
                let p1 = hs.polygon[1];
                let p2 = hs.polygon[2];
                let p3 = hs.polygon[3];
                let hovering = hs.contains(hover_norm);
                let (r, g, b, a) = match (hs.kind, hovering) {
                    (HotspotKind::Talk, true) => (0.3, 0.95, 0.45, 0.45),
                    (HotspotKind::Talk, false) => (0.2, 0.75, 0.35, 0.28),
                    (HotspotKind::Use, true) => (0.95, 0.75, 0.2, 0.5),
                    (HotspotKind::Use, false) => (0.85, 0.6, 0.15, 0.32),
                    (_, true) => (0.45, 0.7, 1.0, 0.42),
                    (_, false) => (0.35, 0.55, 0.9, 0.26),
                };
                batcher.push(DrawElement {
                    layer: 2,
                    shader: ShaderKind::Sprite,
                    effect: DrawEffect::NONE,
                    texture: white,
                    uv: UvRect::FULL,
                    tint: Tint::rgba(r, g, b, a),
                    positions: vec![
                        Vec2::new(p0.x * w, p0.y * h),
                        Vec2::new(p1.x * w, p1.y * h),
                        Vec2::new(p2.x * w, p2.y * h),
                        Vec2::new(p0.x * w, p0.y * h),
                        Vec2::new(p2.x * w, p2.y * h),
                        Vec2::new(p3.x * w, p3.y * h),
                    ],
                    uvs: vec![
                        Vec2::ZERO,
                        Vec2::new(1.0, 0.0),
                        Vec2::new(1.0, 1.0),
                        Vec2::ZERO,
                        Vec2::new(1.0, 1.0),
                        Vec2::new(0.0, 1.0),
                    ],
                });
                if hovering {
                    let label = Self::hotspot_label(hs.id.as_str());
                    let lx = p0.x * w;
                    let ly = (p0.y * h - 18.0).max(STATUS_H + 4.0);
                    draw_text(
                        &mut ui_elements,
                        white,
                        lx,
                        ly,
                        label,
                        2.0,
                        Tint::rgba(1.0, 1.0, 0.85, 1.0),
                        UI_LAYER + 6,
                    );
                }
            }
        }

        // Chrome + dialog rects (white texel)
        for el in ui_elements {
            let mut el = el;
            el.texture = white;
            el.layer = el.layer.max(10);
            batcher.push(el);
        }
        let batches = batcher.finish();
        let clear = if self.bg_tex.is_some() {
            wgpu::Color {
                r: 0.02,
                g: 0.03,
                b: 0.05,
                a: 1.0,
            }
        } else {
            wgpu::Color {
                r: 0.07,
                g: 0.08,
                b: 0.10,
                a: 1.0,
            }
        };
        let (Some(surface), Some(r)) = (self.surface.as_ref(), self.renderer.as_mut()) else {
            return;
        };
        let _ = r.render_frame(surface, view_proj, &batches, clear);

        if advance {
            let _ = self.host.runner.advance(
                &self.host.dialog,
                &self.host.host,
                &mut self.host.vars,
                &mut self.host.tags,
            );
        }
        if let Some(src) = pick_src {
            let _ = self.host.runner.choose(
                &self.host.dialog,
                &self.host.host,
                &mut self.host.vars,
                &mut self.host.tags,
                src,
            );
        }
        self.host.finish_dialog_if_done();
        self.click_this_frame = None;
        self.chrome_ate_click = false;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("Shawshank PAC — Cell Block C (example-08)")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720));
        let Ok(window) = event_loop.create_window(attrs) else {
            return;
        };
        let window = Arc::new(window);
        let surface = self
            .instance
            .create_surface(Arc::clone(&window))
            .expect("surface");
        let adapter = pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("adapter");
        let mut renderer =
            pollster::block_on(WgpuRenderer::new_for_surface(&surface, &self.instance))
                .expect("renderer");
        // White texel always available for UI / hotspot fills.
        let white = renderer
            .upload_texture(1, 1, &[255u8, 255, 255, 255])
            .expect("white texel");
        // Content-pack stills — optional; missing files never panic.
        let bg = try_upload_png(&mut renderer, ASSET_BG);
        let port_red = try_upload_png(&mut renderer, ASSET_PORT_RED);
        let port_andy = try_upload_png(&mut renderer, ASSET_PORT_ANDY);
        if bg.is_none() {
            tracing::warn!("no {ASSET_BG} — drawing dim hotspots only");
        }
        let size = window.inner_size();
        renderer.configure_surface(&surface, &adapter, (size.width, size.height));
        self.window = Some(window);
        self.surface = Some(surface);
        self.adapter = Some(adapter);
        self.renderer = Some(renderer);
        self.white_tex = Some(white);
        self.bg_tex = bg;
        self.port_red = port_red;
        self.port_andy = port_andy;
        tracing::info!("{}", self.host.status);
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
                self.cursor_pos = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.click_this_frame = Some(self.cursor_pos);
                // World hit-test immediately; chrome buttons resolve on next
                // render via UiInput (same click snapshot).
                self.click_world();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key,
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => self.handle_key(&logical_key),
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
        self.last_frame = Instant::now();
    }
}

/// Oracle playtest → JSON report (Holo 3.1 / CUA can emit the same shape later).
fn run_playtest_report(path: &Path) {
    let (result, ms) = timed(|| {
        let mut tasks = Vec::new();

        // gate: N before look
        {
            let mut host = FlagHost::new(load_dialog());
            host.dispatch_action("next_day");
            let ok = !host.andy_arrived()
                && host.status.contains("Examine Red's cell first");
            tasks.push(TaskResult {
                id: "gate_n_before_look",
                description: "Next day rejected before examining Red's cell",
                passed: ok,
                steps: 1,
                detail: host.status.clone(),
            });
        }

        // full spine
        let (walk_ok, walk_detail) = match walkthrough() {
            Ok(()) => (true, "spine complete".into()),
            Err(e) => (false, e),
        };
        tasks.push(TaskResult {
            id: "quest_cellblock_spine",
            description: "Look Red → Next day → Look Andy → Talk → Loose stone",
            passed: walk_ok,
            steps: 8,
            detail: walk_detail,
        });

        // chrome self-check (static contract for discoverability)
        let chrome_ok = STATUS_H >= 40.0 && VERB_H >= 48.0;
        tasks.push(TaskResult {
            id: "chrome_layout",
            description: "Status strip + verb bar chrome constants present",
            passed: chrome_ok,
            steps: 0,
            detail: format!("STATUS_H={STATUS_H} VERB_H={VERB_H}"),
        });

        let rubric = score_rubric(&tasks, chrome_ok);
        PlaytestReport {
            game: "shawshank_cellblock",
            mode: "oracle",
            elapsed_ms: 0, // filled below
            tasks,
            rubric,
        }
    });
    let mut report = result;
    report.elapsed_ms = ms;
    if let Err(e) = report.write_to(path) {
        eprintln!("write playtest report: {e}");
        std::process::exit(1);
    }
    println!("{}", report.to_json());
    println!(
        "playtest {} → {} (overall={:.2})",
        if report.passed() { "PASS" } else { "FAIL" },
        path.display(),
        report.rubric.overall
    );
    if !report.passed() {
        std::process::exit(1);
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--playtest") {
        let path = args
            .get(i + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("shawshank_playtest.json"));
        run_playtest_report(&path);
        return;
    }

    match walkthrough() {
        Ok(()) => tracing::info!("walkthrough OK"),
        Err(e) => {
            eprintln!("walkthrough FAILED: {e}");
            std::process::exit(1);
        }
    }

    if args.iter().any(|a| a == "--headless") {
        return;
    }

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

    #[test]
    fn missing_png_does_not_panic() {
        assert!(load_rgba_png("examples/06-shawshank-pac/assets/__missing__.png").is_none());
        assert!(try_repo_file("examples/06-shawshank-pac/assets/__missing__.png").is_none());
    }

    #[test]
    fn content_pack_pngs_decode_when_present() {
        // Present in the repo content pack; if a stripped checkout omits them,
        // this test still passes (optional art — headless path does not need them).
        if try_repo_file(ASSET_BG).is_some() {
            let (w, h, rgba) = load_rgba_png(ASSET_BG).expect("decode cellblock_bg");
            assert!(w >= 640 && h >= 360, "bg {w}x{h}");
            assert_eq!(rgba.len(), (w * h * 4) as usize);
        }
        if try_repo_file(ASSET_PORT_RED).is_some() {
            let (w, h, rgba) = load_rgba_png(ASSET_PORT_RED).expect("decode portrait_red");
            assert_eq!(w, h);
            assert_eq!(rgba.len(), (w * h * 4) as usize);
        }
        if try_repo_file(ASSET_PORT_ANDY).is_some() {
            let (w, h, _) = load_rgba_png(ASSET_PORT_ANDY).expect("decode portrait_andy");
            assert_eq!(w, h);
        }
    }

    #[test]
    fn textured_quad_has_two_tris() {
        let e = textured_quad(
            TextureId::FIRST,
            0.0,
            0.0,
            10.0,
            20.0,
            0,
            Tint::IDENTITY,
        );
        assert_eq!(e.positions.len(), 6);
        assert_eq!(e.uvs.len(), 6);
        assert_eq!(e.texture, TextureId::FIRST);
    }
}
