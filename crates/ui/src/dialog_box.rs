//! [`DialogBox`] — retained widget that renders a [`DialogRunner`].
//!
//! The widget owns no state — it borrows the runner + game state each
//! frame and emits draw elements + reports the user's selection.
//!
//! Layout: a panel docked to the bottom of the viewport, holding:
//!   * Speaker name (not rendered here — text is deferred to a later
//!     phase; the example logs it via tracing)
//!   * Speaker line (same — log only)
//!   * Up to N choices as buttons (one per [`VisibleChoice`])

use adventure_core::math::Vec2;
use adventure_dialogue::{DialogRunner, DialogTree, VisibleChoice};
use adventure_render2d::DrawElement;
use adventure_scripting::ScriptHost;
use adventure_state::{Tags, VarTable};

use crate::context::UiContext;
use crate::input::UiInput;
use crate::layout::{place, Anchor, Rect};

/// Result of a single [`DialogBox::draw`] call.
#[derive(Debug, Clone, Default)]
pub struct DialogBoxOutput {
    /// The choice that was clicked this frame, if any (its index in
    /// the visible-choices list — *not* the source index in the original
    /// node's choices). Use this to drive `DialogRunner::choose`.
    pub picked_visible_index: Option<usize>,
    /// True if the user clicked somewhere inside the "Continue" hit
    /// area on a linear node (the runner should `advance`).
    pub advance_requested: bool,
    /// Snapshot of the visible choices drawn this frame.
    pub visible_choices: Vec<VisibleChoice>,
    /// True if the runner reports finished.
    pub finished: bool,
    /// Speaker name this frame (for the caller to log / render as text).
    pub speaker: Option<String>,
    /// Speaker line this frame.
    pub line: Option<String>,
}

/// Configuration for the dialog box.
#[derive(Debug, Clone, Copy)]
pub struct DialogBoxConfig {
    /// Viewport size in pixels.
    pub viewport: Vec2,
    /// Margin from the bottom of the viewport.
    pub bottom_margin: f32,
    /// Side margin (left/right).
    pub side_margin: f32,
    /// Per-choice button height.
    pub choice_height: f32,
    /// Spacing between buttons.
    pub choice_spacing: f32,
    /// Spacing between the line area and the choices.
    pub line_to_choices: f32,
    /// Minimum body height (for the line + a few choices).
    pub min_height: f32,
    /// Border thickness.
    pub border: f32,
}

impl Default for DialogBoxConfig {
    fn default() -> Self {
        Self {
            viewport: Vec2::new(800.0, 600.0),
            bottom_margin: 20.0,
            side_margin: 40.0,
            choice_height: 28.0,
            choice_spacing: 4.0,
            line_to_choices: 12.0,
            min_height: 140.0,
            border: 2.0,
        }
    }
}

/// A retained dialog box widget. Stateless across frames — re-built
/// each frame from the runner + game state.
pub struct DialogBox {
    cfg: DialogBoxConfig,
}

impl DialogBox {
    /// Construct with the supplied config.
    pub fn new(cfg: DialogBoxConfig) -> Self {
        Self { cfg }
    }

    /// Construct with default config.
    pub fn default_config() -> Self {
        Self::new(DialogBoxConfig::default())
    }

    /// Compute the body rect (bottom of viewport).
    pub fn body_rect(&self) -> Rect {
        let cfg = self.cfg;
        let w = cfg.viewport.x - 2.0 * cfg.side_margin;
        let h = cfg.min_height;
        let parent = Rect::new(Vec2::ZERO, cfg.viewport);
        place(parent, Vec2::new(w, h), Anchor::BottomCenter, cfg.bottom_margin)
    }

    /// Draw the dialog box for this frame. Returns the user's selection
    /// (if any) and a snapshot of the visible choices.
    ///
    /// `elements` is appended to — clear it before calling if you want
    /// a fresh list.
    pub fn draw(
        &self,
        elements: &mut Vec<DrawElement>,
        input: &UiInput,
        runner: &DialogRunner,
        tree: &DialogTree,
        host: &ScriptHost,
        vars: &VarTable,
        tags: &Tags,
    ) -> DialogBoxOutput {
        let mut out = DialogBoxOutput::default();
        out.finished = runner.is_finished();

        if runner.is_finished() {
            return out;
        }

        let Some(node) = runner.current(tree) else {
            return out;
        };

        out.speaker = Some(node.speaker.to_string());
        out.line = Some(node.text.to_string());

        let body = self.body_rect();

        // Panel (1 fill + 4 border strips = 5 elements).
        let mut ui = UiContext::new(elements, input);
        ui.panel(
            body,
            crate::context::palette::PANEL_BG,
            crate::context::palette::PANEL_BORDER,
            self.cfg.border,
            crate::context::UI_LAYER,
        );

        // Inner content area.
        let inner = body.shrink(self.cfg.border + 6.0);

        let visible = runner.visible_choices(tree, host, vars, tags);
        out.visible_choices = visible.clone();

        if visible.is_empty() {
            // Linear / terminal — use the panel as a "click to continue" hit box.
            // Highlight slightly when hovered.
            if input.hovering(inner) {
                ui.rect(
                    inner,
                    TINT_HOVER_FAINT,
                    crate::context::UI_LAYER + 2,
                );
            }
            if input.clicked_in(inner) {
                out.advance_requested = true;
            }
            return out;
        }

        // Branching — lay out choices vertically from the bottom of inner.
        let total_h = visible.len() as f32 * self.cfg.choice_height
            + (visible.len().saturating_sub(1)) as f32 * self.cfg.choice_spacing;
        let start_y = inner.bottom() - total_h;

        for (i, c) in visible.iter().enumerate() {
            let y = start_y + i as f32 * (self.cfg.choice_height + self.cfg.choice_spacing);
            let r = Rect::new(
                Vec2::new(inner.left(), y),
                Vec2::new(inner.w(), self.cfg.choice_height),
            );
            let state = ui.button(r, crate::context::UI_LAYER + 2);
            // We only need the click; the button fn already pushed bg + border.
            if state.clicked {
                out.picked_visible_index = Some(i);
            }
            // Stash the choice text on the output for the caller to render.
            // (text rendering is a later phase — caller logs it)
            let _ = &c.text;
        }
        out
    }
}

use adventure_render2d::Tint;

/// Faint hover highlight for the panel body on linear nodes.
const TINT_HOVER_FAINT: Tint = Tint(glam::Vec4::new(0.3, 0.3, 0.35, 0.25));

#[cfg(test)]
mod tests {
    use super::*;
    use adventure_dialogue::{Choice, DialogNode};
    use adventure_scripting::ScriptHost;
    use adventure_state::{Tags, VarTable};
    use std::collections::BTreeMap;

    fn two_choice_tree() -> DialogTree {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "ask".into(),
            DialogNode::branching(
                "ask",
                "Bob",
                "Pick one:",
                vec![
                    Choice::new("A", None::<&str>),
                    Choice::new("B", None::<&str>),
                ],
            ),
        );
        DialogTree {
            id: "t".into(),
            entry: "ask".into(),
            nodes,
        }
    }

    fn linear_tree() -> DialogTree {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "intro".into(),
            DialogNode::linear("intro", "Bob", "Hello.", None::<&str>),
        );
        DialogTree {
            id: "t".into(),
            entry: "intro".into(),
            nodes,
        }
    }

    #[test]
    fn body_rect_sits_at_bottom_of_viewport() {
        let d = DialogBox::default_config();
        let body = d.body_rect();
        assert_eq!(body.bottom(), 600.0 - 20.0);
        assert_eq!(body.left(), 40.0);
        assert_eq!(body.right(), 800.0 - 40.0);
    }

    #[test]
    fn draw_branching_reports_two_choices() {
        let t = two_choice_tree();
        let host = ScriptHost::new();
        let v = VarTable::new();
        let tags = Tags::new();
        let mut runner = DialogRunner::new(&t);
        runner.start(&t, &host, &mut VarTable::new(), &mut Tags::new()).unwrap();

        let mut elems = Vec::new();
        let input = UiInput::default();
        let dbox = DialogBox::default_config();
        let out = dbox.draw(&mut elems, &input, &runner, &t, &host, &v, &tags);

        assert_eq!(out.visible_choices.len(), 2);
        assert_eq!(out.speaker.as_deref(), Some("Bob"));
        assert_eq!(out.line.as_deref(), Some("Pick one:"));
        assert!(out.picked_visible_index.is_none());
    }

    #[test]
    fn draw_branching_reports_pick_when_clicked() {
        let t = two_choice_tree();
        let host = ScriptHost::new();
        let v = VarTable::new();
        let tags = Tags::new();
        let mut runner = DialogRunner::new(&t);
        runner.start(&t, &host, &mut VarTable::new(), &mut Tags::new()).unwrap();

        let cfg = DialogBoxConfig::default();
        let body = DialogBox::new(cfg).body_rect();
        let inner = body.shrink(cfg.border + 6.0);
        // Two choices stacked at the bottom of inner.
        let total_h = 2.0 * cfg.choice_height + cfg.choice_spacing;
        let start_y = inner.bottom() - total_h;
        // Click the top of the second choice.
        let click_y = start_y + cfg.choice_height + cfg.choice_spacing + 5.0;
        let click = Vec2::new(inner.center().x, click_y);

        let mut elems = Vec::new();
        let input = UiInput::new(click, Some(click));
        let dbox = DialogBox::default_config();
        let out = dbox.draw(&mut elems, &input, &runner, &t, &host, &v, &tags);

        assert_eq!(out.picked_visible_index, Some(1));
    }

    #[test]
    fn draw_linear_requests_advance_on_click() {
        let t = linear_tree();
        let host = ScriptHost::new();
        let v = VarTable::new();
        let tags = Tags::new();
        let mut runner = DialogRunner::new(&t);
        runner.start(&t, &host, &mut VarTable::new(), &mut Tags::new()).unwrap();

        let cfg = DialogBoxConfig::default();
        let body = DialogBox::new(cfg).body_rect();
        let inner = body.shrink(cfg.border + 6.0);
        let click = inner.center();

        let mut elems = Vec::new();
        let input = UiInput::new(click, Some(click));
        let dbox = DialogBox::default_config();
        let out = dbox.draw(&mut elems, &input, &runner, &t, &host, &v, &tags);

        assert!(out.advance_requested);
    }

    #[test]
    fn draw_finished_runner_outputs_finished() {
        let t = linear_tree();
        let host = ScriptHost::new();
        let v = VarTable::new();
        let tags = Tags::new();
        let mut runner = DialogRunner::new(&t);
        runner.start(&t, &host, &mut VarTable::new(), &mut Tags::new()).unwrap();
        // Linear terminal node → advance finishes the conversation.
        runner.advance(&t, &host, &mut VarTable::new(), &mut Tags::new()).unwrap();
        assert!(runner.is_finished());

        let mut elems = Vec::new();
        let input = UiInput::default();
        let dbox = DialogBox::default_config();
        let out = dbox.draw(&mut elems, &input, &runner, &t, &host, &v, &tags);

        assert!(out.finished);
        assert!(out.speaker.is_none());
    }
}
