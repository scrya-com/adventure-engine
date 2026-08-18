//! Story scripts — the VN layer (Phase 8B, ADR 0008).
//!
//! Authored, sequential, cutscene-style stories running on top of the
//! engine's existing crates:
//!
//! * [`Story`] — `.story.ron`: labels of ordered statements (`Say`, `Scene`,
//!   `Menu`, `If`, `Jump`, `Call`, …). Flow is data; all *logic* stays Rhai
//!   via `adventure-scripting` (ADR 0004 / 0005).
//! * [`StoryRunner`] — a deterministic state machine shaped like
//!   `DialogRunner`: runs statements until the next blocking one, driven by
//!   `advance` / `choose`. Presentation statements emit [`Action`]s for the
//!   host; the runner never touches ECS / render / audio.
//! * [`StoryPosition`] — pure-data position (`ip` + call stack), ready for
//!   versioned saves (8B.3) and replay labels (8B.4).
//!
//! Design: `docs/VN_LAYER_DESIGN.md` · ADR: `docs/DECISIONS/0008-story-script-layer.md`.

#![deny(missing_docs)]

mod compile;
pub mod error;
pub mod runner;
pub mod stmt;

pub use error::ScenarioError;
pub use runner::{Action, StepResult, StoryPosition, StoryRunner, VisibleChoice};
pub use stmt::{
    Anchor, Channel, EndingSpec, HideSpec, IfSpec, InputSpec, MenuSpec, PauseSpec, PlaySpec,
    SaySpec, SceneSpec, ShowSpec, Stmt, StopSpec, Story, StoryChoice, Transition,
};
