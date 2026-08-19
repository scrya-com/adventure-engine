//! [`StoryRunner`] — push-button state machine over a compiled [`Story`].
//!
//! Mirrors `DialogRunner`'s shape (crates/dialogue): the host drives
//! `start` → `StepResult` → `advance`/`choose` until `Finished`. The runner
//! never touches ECS / render / audio — presentation statements emit
//! [`Action`]s the caller drains and applies, keeping the crate headless
//! and deterministic.

use adventure_core::SmolStr;
use adventure_scripting::ScriptHost;
use adventure_state::{Tags, VarTable};
use serde::{Deserialize, Serialize};

use crate::compile::{compile, Compiled, Ir};
use crate::stmt::{Anchor, Channel, MenuSpec, Story, Transition};

/// A presentation command emitted by the runner for the host to apply.
///
/// The runner stays pure data-in/data-out; mapping these onto render2d /
/// audio is the engine application's job.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Change the background (host clears the sprite layer).
    Scene {
        /// Background asset path.
        bg: SmolStr,
        /// Optional transition.
        with: Option<Transition>,
    },
    /// Show a sprite.
    Show {
        /// Sprite asset path.
        sprite: SmolStr,
        /// Screen anchor.
        at: Option<Anchor>,
        /// Replacement tag (`None` = use the sprite path).
        as_: Option<SmolStr>,
        /// Optional transition.
        with: Option<Transition>,
    },
    /// Hide a shown sprite.
    Hide {
        /// Which sprite to hide (`None` = all).
        as_: Option<SmolStr>,
        /// Optional transition.
        with: Option<Transition>,
    },
    /// Start an audio asset on a channel.
    Play {
        /// Target channel.
        channel: Channel,
        /// Audio asset path.
        asset: SmolStr,
        /// Linear volume 0..1.
        volume: Option<f32>,
        /// Fade-in seconds.
        fade_in: Option<f32>,
        /// Loop the asset.
        loop_: bool,
    },
    /// Stop a channel.
    Stop {
        /// Channel to stop.
        channel: Channel,
        /// Fade-out seconds.
        fade_out: Option<f32>,
    },
    /// Looping (or one-shot) movie — Scene/Show path ending in `.webm`.
    Movie {
        /// Filesystem or authored path (host may still run it through the resolver).
        path: SmolStr,
        /// Loop like Ren'Py `Movie()`.
        loop_: bool,
    },
}

/// Where the runner is inside the story — pure data, ready for saves (8B.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryPosition {
    /// Instruction pointer into the compiled flat program.
    ip: u32,
    /// Return addresses for `Call` (empty = bottom of the stack).
    call_stack: Vec<u32>,
}

impl StoryPosition {
    /// Reconstruct a position from a save (ip is one past the blocking stmt).
    pub fn from_parts(ip: u32, call_stack: Vec<u32>) -> Self {
        Self { ip, call_stack }
    }

    /// Current instruction pointer.
    pub fn ip(&self) -> u32 {
        self.ip
    }

    /// Return-address stack (Call/Return).
    pub fn call_stack(&self) -> &[u32] {
        &self.call_stack
    }

    /// Return-address stack depth (for debugging / save UIs).
    pub fn call_depth(&self) -> usize {
        self.call_stack.len()
    }
}

/// One menu choice (locked rows stay visible so the player can see the gate).
#[derive(Clone, Debug, PartialEq)]
pub struct VisibleChoice {
    /// Index into the authored `MenuSpec::choices` list — pass this back to
    /// [`StoryRunner::choose`].
    pub source_index: usize,
    /// Player-facing choice text.
    pub text: String,
    /// True when the Rhai condition failed (fail-closed).
    pub locked: bool,
    /// Raw condition text (`desire >= 4`) when locked.
    pub lock_reason: Option<String>,
}

fn collect_menu_choices(
    spec: &MenuSpec,
    host: &ScriptHost,
    vars: &VarTable,
    tags: &Tags,
) -> Vec<VisibleChoice> {
    spec.choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let open = match c.condition.as_deref() {
                Some(expr) => host.eval_condition(expr, vars, tags).unwrap_or(false),
                None => true,
            };
            VisibleChoice {
                source_index: i,
                text: c.text.to_string(),
                locked: !open,
                lock_reason: if open {
                    None
                } else {
                    c.condition.clone()
                },
            }
        })
        .collect()
}

/// The blocking point a [`StoryRunner`] stopped at.
#[derive(Clone, Debug, PartialEq)]
pub enum StepResult {
    /// A dialogue line (speaker `None` = narrator). Advance to continue.
    Say {
        /// Speaker character id.
        who: Option<SmolStr>,
        /// Line text (markup resolved by the `ui` crate, 8B.2).
        text: String,
    },
    /// A choice menu. Choose via [`StoryRunner::choose`].
    Menu {
        /// Optional prompt above the choices.
        prompt: Option<String>,
        /// Visible choices (conditions applied, fail-closed).
        choices: Vec<VisibleChoice>,
    },
    /// A timed wait. Advance to continue.
    Pause {
        /// Seconds.
        seconds: f32,
    },
    /// Name-entry prompt. Continue via [`StoryRunner::submit_text`].
    Input {
        /// Prompt shown above the field.
        prompt: String,
        /// Suggested / fallback value.
        default: SmolStr,
        /// `VarTable` key that will be written.
        var: SmolStr,
    },
    /// The story ended.
    Finished {
        /// Ending id if it ended via an `Ending` statement.
        ending: Option<SmolStr>,
    },
}

/// What the runner is currently blocked on (internal bookkeeping).
#[derive(Clone, Debug)]
enum Blocked {
    /// Blocked on a `Say`.
    Say,
    /// Blocked on a `Pause`.
    Pause,
    /// Blocked on a `Menu` (spec stashed for `choose`).
    Menu(MenuSpec),
    /// Blocked on `Input` (var + default stashed for `submit_text`).
    Input { var: SmolStr, default: SmolStr },
}

/// State machine over a [`Story`].
///
/// Lifecycle:
///   1. [`StoryRunner::new`] — compile + validate (all errors up front).
///   2. [`StoryRunner::start`] — enter the entry label, run to first block.
///   3. Loop: consume [`StepResult`], call `advance` (Say/Pause) or
///      `choose` (Menu) until `Finished`.
///   4. Drain [`Action`]s after each step and apply them to the presentation
///      layer (order is significant).
#[derive(Clone, Debug)]
pub struct StoryRunner {
    story_id: String,
    entry: SmolStr,
    program: Compiled,
    position: Option<StoryPosition>,
    finished: bool,
    blocked: Option<Blocked>,
    pending: Vec<Action>,
    history: Vec<SmolStr>,
}

impl StoryRunner {
    /// Compile + validate a story and build a runner for it.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ScenarioError::Validation`] (all issues at once) if
    /// labels dangle or any Rhai snippet fails to parse.
    pub fn new(story: &Story) -> Result<Self, crate::ScenarioError> {
        let program = compile(story)?;
        let entry = program
            .labels
            .get_key_value(&story.entry)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| story.entry.clone());
        Ok(Self {
            story_id: story.id.clone(),
            entry,
            program,
            position: None,
            finished: false,
            blocked: None,
            pending: Vec::new(),
            history: Vec::new(),
        })
    }

    /// The story id this runner was built from.
    pub fn story_id(&self) -> &str {
        &self.story_id
    }

    /// True once the story has finished.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Current position (`None` before `start` / after `Finished`).
    pub fn position(&self) -> Option<&StoryPosition> {
        self.position.as_ref()
    }

    /// Labels entered so far (entry + jump/call/goto targets), in order.
    pub fn history(&self) -> &[SmolStr] {
        &self.history
    }

    /// Resume at a saved [`StoryPosition`] and re-emit the blocking step.
    ///
    /// Saved `ip` is one past the blocking instruction (same as after
    /// `start` / `advance`). Presentation (`Action`s) is **not** replayed —
    /// the host must restore backgrounds / music from the save body.
    ///
    /// # Errors
    ///
    /// [`crate::ScenarioError::NotStarted`] if `ip` is past the program.
    pub fn restore(
        &mut self,
        pos: StoryPosition,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        if pos.ip as usize > self.program.instrs.len() {
            return Err(crate::ScenarioError::NotStarted);
        }
        self.position = Some(pos.clone());
        self.finished = false;
        self.blocked = None;
        self.pending.clear();
        if pos.ip == 0 {
            return self.run_until_blocking(host, vars, tags);
        }
        let blocked_ip = (pos.ip - 1) as usize;
        match &self.program.instrs[blocked_ip] {
            Ir::Say { who, text } => {
                self.blocked = Some(Blocked::Say);
                Ok(StepResult::Say {
                    who: who.clone(),
                    text: text.to_string(),
                })
            }
            Ir::Pause { seconds } => {
                self.blocked = Some(Blocked::Pause);
                Ok(StepResult::Pause { seconds: *seconds })
            }
            Ir::Menu(spec) => {
                let prompt = spec.prompt.as_deref().map(str::to_string);
                let choices = collect_menu_choices(spec, host, vars, tags);
                self.blocked = Some(Blocked::Menu(spec.clone()));
                Ok(StepResult::Menu { prompt, choices })
            }
            Ir::Input(spec) => {
                let default = spec.default.clone().unwrap_or_default();
                self.blocked = Some(Blocked::Input {
                    var: spec.var.clone(),
                    default: default.clone(),
                });
                Ok(StepResult::Input {
                    prompt: spec.prompt.to_string(),
                    default,
                    var: spec.var.clone(),
                })
            }
            _ => self.run_until_blocking(host, vars, tags),
        }
    }

    /// Take the presentation actions emitted since the last drain.
    pub fn drain_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.pending)
    }

    /// Start the story at its entry label.
    ///
    /// # Errors
    ///
    /// Propagates Rhai failures from `Exec` / `If` conditions on the way.
    pub fn start(
        &mut self,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        let entry = self.entry.clone();
        self.start_at_inner(&entry, host, vars, tags)
    }

    /// Start at an arbitrary label (replay-gallery entry point, 8B.4).
    ///
    /// # Errors
    ///
    /// [`crate::ScenarioError::DanglingLabel`] if the label doesn't exist;
    /// propagates Rhai failures otherwise.
    pub fn start_at(
        &mut self,
        label: &str,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        self.start_at_inner(label, host, vars, tags)
    }

    fn start_at_inner(
        &mut self,
        label: &str,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        let offset = *self
            .program
            .labels
            .get(label)
            .ok_or_else(|| crate::ScenarioError::DanglingLabel(label.to_string()))?;
        self.position = Some(StoryPosition {
            ip: offset,
            call_stack: Vec::new(),
        });
        self.finished = false;
        self.blocked = None;
        self.pending.clear();
        self.history.clear();
        self.history.push(label.into());
        self.run_until_blocking(host, vars, tags)
    }

    /// Continue from a `Say` or `Pause`.
    ///
    /// # Errors
    ///
    /// [`crate::ScenarioError::BlockedAtMenu`] if waiting on a menu;
    /// `Finished` / `NotStarted` otherwise out of contract.
    pub fn advance(
        &mut self,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        if self.finished {
            return Err(crate::ScenarioError::Finished);
        }
        if self.position.is_none() {
            return Err(crate::ScenarioError::NotStarted);
        }
        match self.blocked.take() {
            Some(Blocked::Menu(spec)) => {
                self.blocked = Some(Blocked::Menu(spec));
                Err(crate::ScenarioError::BlockedAtMenu)
            }
            Some(Blocked::Input { var, default }) => {
                self.blocked = Some(Blocked::Input { var, default });
                Err(crate::ScenarioError::BlockedAtInput)
            }
            Some(_) => self.run_until_blocking(host, vars, tags),
            None => Err(crate::ScenarioError::NotStarted),
        }
    }

    /// Pick a menu choice by its authored index (see [`VisibleChoice`]).
    ///
    /// Re-evaluates the choice condition (fail-closed), fires its effects,
    /// then transfers to `goto` or falls through past the menu.
    ///
    /// # Errors
    ///
    /// [`crate::ScenarioError::NotBlockedAtMenu`] if not blocked on a menu;
    /// `ChoiceOutOfRange` / `ChoiceUnavailable` for bad indexes; propagates
    /// Rhai failures from effects.
    pub fn choose(
        &mut self,
        source_index: usize,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        if self.finished {
            return Err(crate::ScenarioError::Finished);
        }
        if self.position.is_none() {
            return Err(crate::ScenarioError::NotStarted);
        }
        let spec = match self.blocked.take() {
            Some(Blocked::Menu(spec)) => spec,
            Some(other) => {
                let err = match &other {
                    Blocked::Input { .. } => crate::ScenarioError::BlockedAtInput,
                    _ => crate::ScenarioError::NotBlockedAtMenu,
                };
                self.blocked = Some(other);
                return Err(err);
            }
            None => return Err(crate::ScenarioError::NotBlockedAtMenu),
        };
        let choice = match spec.choices.get(source_index) {
            Some(c) => c,
            None => {
                let err = crate::ScenarioError::ChoiceOutOfRange {
                    index: source_index,
                    len: spec.choices.len(),
                };
                self.blocked = Some(Blocked::Menu(spec));
                return Err(err);
            }
        };
        if let Some(cond) = choice.condition.as_deref() {
            if !host.eval_condition(cond, vars, tags)? {
                self.blocked = Some(Blocked::Menu(spec));
                return Err(crate::ScenarioError::ChoiceUnavailable {
                    index: source_index,
                });
            }
        }
        if let Some(effects) = choice.effects.as_deref() {
            host.run(effects, vars, tags)?;
        }
        if let Some(goto) = choice.goto.as_deref() {
            let offset = *self
                .program
                .labels
                .get(goto)
                .ok_or_else(|| crate::ScenarioError::DanglingLabel(goto.to_string()))?;
            self.history.push(goto.into());
            self.position.as_mut().expect("position checked above").ip = offset;
        }
        self.run_until_blocking(host, vars, tags)
    }

    /// Submit text from a blocking [`StepResult::Input`].
    ///
    /// Strips surrounding whitespace; empty input uses the authored default
    /// (or `""` if none). Writes a string into `vars` under the statement's
    /// `var` key (`kevinname` / `rachelname`).
    ///
    /// # Errors
    ///
    /// [`crate::ScenarioError::NotBlockedAtInput`] if not waiting on input.
    pub fn submit_text(
        &mut self,
        text: &str,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        if self.finished {
            return Err(crate::ScenarioError::Finished);
        }
        if self.position.is_none() {
            return Err(crate::ScenarioError::NotStarted);
        }
        let (var, default) = match self.blocked.take() {
            Some(Blocked::Input { var, default }) => (var, default),
            Some(other) => {
                self.blocked = Some(other);
                return Err(crate::ScenarioError::NotBlockedAtInput);
            }
            None => return Err(crate::ScenarioError::NotBlockedAtInput),
        };
        let trimmed = text.trim();
        let value = if trimmed.is_empty() {
            default
        } else {
            SmolStr::new(trimmed)
        };
        vars.set(var.as_str(), adventure_state::VarValue::S(value));
        self.run_until_blocking(host, vars, tags)
    }

    /// Execute instructions until the next blocking one (or finish).
    fn run_until_blocking(
        &mut self,
        host: &ScriptHost,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<StepResult, crate::ScenarioError> {
        loop {
            let ip = match self.position.as_ref() {
                Some(pos) => pos.ip as usize,
                None => {
                    self.finished = true;
                    self.blocked = None;
                    return Ok(StepResult::Finished { ending: None });
                }
            };
            match &self.program.instrs[ip] {
                Ir::Say { who, text } => {
                    let step = StepResult::Say {
                        who: who.clone(),
                        text: text.to_string(),
                    };
                    self.position.as_mut().expect("position checked above").ip += 1;
                    self.blocked = Some(Blocked::Say);
                    return Ok(step);
                }
                Ir::Pause { seconds } => {
                    let step = StepResult::Pause { seconds: *seconds };
                    self.position.as_mut().expect("position checked above").ip += 1;
                    self.blocked = Some(Blocked::Pause);
                    return Ok(step);
                }
                Ir::Menu(spec) => {
                    let prompt = spec.prompt.as_deref().map(str::to_string);
                    let choices = collect_menu_choices(spec, host, vars, tags);
                    let step = StepResult::Menu { prompt, choices };
                    self.position.as_mut().expect("position checked above").ip += 1;
                    self.blocked = Some(Blocked::Menu(spec.clone()));
                    return Ok(step);
                }
                Ir::Scene(spec) => {
                    self.pending.push(Action::Scene {
                        bg: spec.bg.clone(),
                        with: spec.with.clone(),
                    });
                    if path_is_movie(spec.bg.as_str()) {
                        self.pending.push(Action::Movie {
                            path: spec.bg.clone(),
                            loop_: true,
                        });
                    }
                    self.position.as_mut().expect("position checked above").ip += 1;
                }
                Ir::Show(spec) => {
                    self.pending.push(Action::Show {
                        sprite: spec.sprite.clone(),
                        at: spec.at.clone(),
                        as_: spec.as_.clone(),
                        with: spec.with.clone(),
                    });
                    if path_is_movie(spec.sprite.as_str()) {
                        self.pending.push(Action::Movie {
                            path: spec.sprite.clone(),
                            loop_: true,
                        });
                    }
                    self.position.as_mut().expect("position checked above").ip += 1;
                }
                Ir::Hide(spec) => {
                    self.pending.push(Action::Hide {
                        as_: spec.as_.clone(),
                        with: spec.with.clone(),
                    });
                    self.position.as_mut().expect("position checked above").ip += 1;
                }
                Ir::Play(spec) => {
                    self.pending.push(Action::Play {
                        channel: spec.channel.clone(),
                        asset: spec.asset.clone(),
                        volume: spec.volume,
                        fade_in: spec.fade_in,
                        loop_: spec.loop_,
                    });
                    self.position.as_mut().expect("position checked above").ip += 1;
                }
                Ir::Stop(spec) => {
                    self.pending.push(Action::Stop {
                        channel: spec.channel.clone(),
                        fade_out: spec.fade_out,
                    });
                    self.position.as_mut().expect("position checked above").ip += 1;
                }
                Ir::Exec(src) => {
                    host.run(src, vars, tags)?;
                    self.position.as_mut().expect("position checked above").ip += 1;
                }
                Ir::JumpIfFalse { cond, target } => {
                    let ok = host.eval_condition(cond, vars, tags)?;
                    let next = if ok { (ip + 1) as u32 } else { *target };
                    self.position.as_mut().expect("position checked above").ip = next;
                }
                Ir::Jump(target) => {
                    if let Some(name) = self.program.label_offsets.get(target) {
                        let name = name.clone();
                        self.history.push(name);
                    }
                    self.position.as_mut().expect("position checked above").ip = *target;
                }
                Ir::Call(target) => {
                    if let Some(name) = self.program.label_offsets.get(target) {
                        let name = name.clone();
                        self.history.push(name);
                    }
                    let pos = self.position.as_mut().expect("position checked above");
                    pos.call_stack.push((ip + 1) as u32);
                    pos.ip = *target;
                }
                Ir::Return => {
                    let pos = self.position.as_mut().expect("position checked above");
                    match pos.call_stack.pop() {
                        Some(ret) => pos.ip = ret,
                        None => {
                            self.position = None;
                            self.finished = true;
                            self.blocked = None;
                            return Ok(StepResult::Finished { ending: None });
                        }
                    }
                }
                Ir::Ending { id } => {
                    let step = StepResult::Finished { ending: id.clone() };
                    self.position = None;
                    self.finished = true;
                    self.blocked = None;
                    return Ok(step);
                }
                Ir::Input(spec) => {
                    let default = spec.default.clone().unwrap_or_default();
                    let step = StepResult::Input {
                        prompt: spec.prompt.to_string(),
                        default: default.clone(),
                        var: spec.var.clone(),
                    };
                    self.position.as_mut().expect("position checked above").ip += 1;
                    self.blocked = Some(Blocked::Input {
                        var: spec.var.clone(),
                        default,
                    });
                    return Ok(step);
                }
            }
        }
    }
}

fn path_is_movie(path: &str) -> bool {
    let lower = path.trim().to_ascii_lowercase();
    lower.ends_with(".webm") || lower.ends_with(".mp4")
}

#[cfg(test)]
mod tests {
    use super::*;
    use adventure_state::Tag;

    /// Kitchen-sink fixture mirroring the design-doc sample.
    fn story() -> Story {
        let src = r#"
(
    id: "fixture",
    entry: "start",
    labels: {
        "start": [
            Scene(( bg: "bg/kitchen_day", with: Dissolve(1.0) )),
            Play(( channel: Music, asset: "music/spark", volume: 0.4 )),
            Narrate("Another day begins."),
            Say(( who: Some("kevin"), text: "Just the usual work stuff." )),
            Exec("set_int(\"day\", 1); add_tag(\"State.Ch1.Started\")"),
            If((
                condition: "has_tag(\"State.Path.Frank\")",
                then: [ Say(( who: Some("rachel"), text: "Why him?" )) ],
                else_: [ Narrate("A quiet day.") ],
            )),
            Menu((
                prompt: Some("What now?"),
                choices: [
                    ( text: "Go left",  effects: Some("set_int(\"desire\", desire + 1)"), goto: Some("left") ),
                    ( text: "Go right", goto: Some("right") ),
                    ( text: "Secret", condition: Some("has_tag(\"State.Ch1.Started\")"), goto: Some("secret") ),
                    ( text: "Locked", condition: Some("day >= 99") ),
                ],
            )),
        ],
        "left": [
            Scene(( bg: "cg/rthm20", with: Fade(0.5) )),
            Pause(( seconds: 0.8 )),
            Ending(( id: Some("gave_in") )),
        ],
        "right": [
            Narrate("She focused."),
            Return,
        ],
        "secret": [
            Say(( who: Some("kevin"), text: "A secret." )),
            Call("helper"),
            Narrate("Back from helper."),
        ],
        "helper": [
            Narrate("Helper ran."),
            Return,
        ],
    },
)
"#;
        Story::from_ron_str(src).unwrap()
    }

    struct Ctx {
        host: ScriptHost,
        vars: VarTable,
        tags: Tags,
    }

    impl Ctx {
        fn new() -> Self {
            let mut vars = VarTable::new();
            vars.set("desire", adventure_state::VarValue::I(0));
            Self {
                host: ScriptHost::new(),
                vars,
                tags: Tags::new(),
            }
        }
    }

    #[test]
    fn start_runs_to_first_say_emitting_actions() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(
            matches!(step, StepResult::Say { who: None, ref text } if text == "Another day begins.")
        );
        let actions = runner.drain_actions();
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], Action::Scene { bg, .. } if bg == "bg/kitchen_day"));
        assert!(matches!(
            &actions[1],
            Action::Play {
                channel: Channel::Music,
                ..
            }
        ));
        assert!(runner.drain_actions().is_empty(), "drain is destructive");
    }

    #[test]
    fn full_run_choice_left_reaches_ending() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        let mut says = 0;
        loop {
            match step {
                StepResult::Say { .. } => {
                    says += 1;
                    step = runner
                        .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                        .unwrap();
                }
                StepResult::Menu { choices, .. } => {
                    assert_eq!(choices.len(), 4, "locked rows stay listed");
                    assert!(!choices[2].locked && choices[2].text == "Secret");
                    assert!(choices[3].locked && choices[3].text == "Locked");
                    step = runner
                        .choose(0, &ctx.host, &mut ctx.vars, &mut ctx.tags)
                        .unwrap();
                }
                StepResult::Pause { seconds } => {
                    assert!((seconds - 0.8).abs() < 1e-6);
                    step = runner
                        .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                        .unwrap();
                }
                StepResult::Finished { ending } => {
                    assert_eq!(ending.as_deref(), Some("gave_in"));
                    break;
                }
                StepResult::Input { .. } => panic!("fixture has no Input"),
            }
        }
        assert!(says >= 3);
        assert!(runner.is_finished());
        assert!(runner.position().is_none());
        // Effects fired (choice) + Exec (label body) before the menu.
        assert_eq!(ctx.vars.get("desire").and_then(|v| v.as_int()), Some(1));
        assert_eq!(ctx.vars.get("day").and_then(|v| v.as_int()), Some(1));
        assert!(ctx.tags.has(&Tag::new("State.Ch1.Started").unwrap()));
        // Presentation actions: scene, play, scene.
        let actions = runner.drain_actions();
        assert_eq!(actions.len(), 3);
        assert!(matches!(&actions[2], Action::Scene { bg, .. } if bg == "cg/rthm20"));
    }

    #[test]
    fn choice_right_returns_and_finishes() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        while let StepResult::Say { .. } = step {
            step = runner
                .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                .unwrap();
        }
        let StepResult::Menu { .. } = step else {
            panic!("expected menu, got {step:?}")
        };
        let step = runner
            .choose(1, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Say { .. }));
        let step = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Finished { ending: None }));
    }

    #[test]
    fn call_returns_to_call_site() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        while let StepResult::Say { .. } = step {
            step = runner
                .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                .unwrap();
        }
        let step = runner
            .choose(2, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Say { who: Some(ref w), .. } if w == "kevin"));
        let step = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Say { who: None, ref text } if text == "Helper ran."));
        let step = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(
            matches!(step, StepResult::Say { who: None, ref text } if text == "Back from helper.")
        );
        // End of "secret" label = implicit Return at stack bottom → finish.
        let step = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Finished { ending: None }));
    }

    #[test]
    fn locked_choice_is_fail_closed() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        while let StepResult::Say { .. } = step {
            step = runner
                .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                .unwrap();
        }
        let err = runner
            .choose(3, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap_err();
        assert!(matches!(
            err,
            crate::ScenarioError::ChoiceUnavailable { index: 3 }
        ));
        let err = runner
            .choose(9, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap_err();
        assert!(matches!(
            err,
            crate::ScenarioError::ChoiceOutOfRange { index: 9, .. }
        ));
    }

    #[test]
    fn advance_on_menu_and_choose_on_say_are_contract_errors() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        while let StepResult::Say { .. } = step {
            step = runner
                .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                .unwrap();
        }
        let err = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap_err();
        assert!(matches!(err, crate::ScenarioError::BlockedAtMenu));

        let mut ctx2 = Ctx::new();
        let mut runner2 = StoryRunner::new(&story()).unwrap();
        let _ = runner2
            .start(&ctx2.host, &mut ctx2.vars, &mut ctx2.tags)
            .unwrap();
        let err = runner2
            .choose(0, &ctx2.host, &mut ctx2.vars, &mut ctx2.tags)
            .unwrap_err();
        assert!(matches!(err, crate::ScenarioError::NotBlockedAtMenu));
    }

    #[test]
    fn after_finished_further_calls_error() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        while let StepResult::Say { .. } = step {
            step = runner
                .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                .unwrap();
        }
        let _ = runner
            .choose(1, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        let step = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Finished { .. }));
        assert!(matches!(
            runner.advance(&ctx.host, &mut ctx.vars, &mut ctx.tags),
            Err(crate::ScenarioError::Finished)
        ));
    }

    #[test]
    fn history_records_label_transfers() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let mut step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        while let StepResult::Say { .. } = step {
            step = runner
                .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
                .unwrap();
        }
        let step = runner
            .choose(2, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Say { .. }));
        // Call("helper") runs after this Say; history records the call target.
        let _ = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        let names: Vec<&str> = runner.history().iter().map(SmolStr::as_str).collect();
        assert_eq!(names, vec!["start", "secret", "helper"]);
    }

    #[test]
    fn position_serializes_for_saves() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let _ = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        let pos = runner.position().unwrap().clone();
        let ser = ron::to_string(&pos).unwrap();
        let back: StoryPosition = ron::from_str(&ser).unwrap();
        assert_eq!(pos, back);
        assert_eq!(back.call_depth(), 0);
    }

    #[test]
    fn start_at_runs_replay_label() {
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story()).unwrap();
        let step = runner
            .start_at("left", &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert!(matches!(step, StepResult::Pause { .. }));
    }

    #[test]
    fn input_writes_var_and_empty_uses_default() {
        let src = r#"
(
    id: "names",
    entry: "start",
    labels: {
        "start": [
            Input(( prompt: "Your name?", var: "kevinname", default: "Kevin" )),
            Input(( prompt: "Partner?", var: "rachelname" )),
            Narrate("done"),
        ],
    },
)
"#;
        let story = Story::from_ron_str(src).unwrap();
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story).unwrap();
        let step = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        match step {
            StepResult::Input {
                prompt,
                default,
                var,
            } => {
                assert_eq!(prompt, "Your name?");
                assert_eq!(default.as_str(), "Kevin");
                assert_eq!(var.as_str(), "kevinname");
            }
            other => panic!("expected Input, got {other:?}"),
        }
        let err = runner
            .advance(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap_err();
        assert!(matches!(err, crate::ScenarioError::BlockedAtInput));
        let step = runner
            .submit_text("  ", &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert_eq!(
            ctx.vars.get("kevinname").and_then(|v| v.as_str()),
            Some("Kevin")
        );
        match step {
            StepResult::Input { var, .. } => assert_eq!(var.as_str(), "rachelname"),
            other => panic!("expected second Input, got {other:?}"),
        }
        let step = runner
            .submit_text("  Rachel  ", &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert_eq!(
            ctx.vars.get("rachelname").and_then(|v| v.as_str()),
            Some("Rachel")
        );
        assert!(matches!(step, StepResult::Say { .. }));
    }

    #[test]
    fn restore_reemits_blocking_input() {
        let src = r#"
(
    id: "names",
    entry: "start",
    labels: {
        "start": [
            Input(( prompt: "Your name?", var: "kevinname", default: "Kevin" )),
            Narrate("hello"),
        ],
    },
)
"#;
        let story = Story::from_ron_str(src).unwrap();
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story).unwrap();
        runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        let pos = runner.position().unwrap().clone();
        let mut runner2 = StoryRunner::new(&story).unwrap();
        let step = runner2
            .restore(pos, &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        match step {
            StepResult::Input { var, default, .. } => {
                assert_eq!(var.as_str(), "kevinname");
                assert_eq!(default.as_str(), "Kevin");
            }
            other => panic!("expected Input, got {other:?}"),
        }
        runner2
            .submit_text("Ada", &ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        assert_eq!(
            ctx.vars.get("kevinname").and_then(|v| v.as_str()),
            Some("Ada")
        );
    }

    #[test]
    fn scene_webm_emits_movie_action() {
        let src = r#"
(
    id: "m",
    entry: "start",
    labels: {
        "start": [
            Scene(( bg: "images/animations/anima1.webm" )),
            Show(( sprite: "clip.webm" )),
            Narrate("ok"),
        ],
    },
)
"#;
        let story = Story::from_ron_str(src).unwrap();
        let mut ctx = Ctx::new();
        let mut runner = StoryRunner::new(&story).unwrap();
        let _ = runner
            .start(&ctx.host, &mut ctx.vars, &mut ctx.tags)
            .unwrap();
        let actions = runner.drain_actions();
        assert!(matches!(&actions[0], Action::Scene { bg, .. } if bg.ends_with(".webm")));
        assert!(
            matches!(&actions[1], Action::Movie { path, loop_ } if path.ends_with("anima1.webm") && *loop_)
        );
        assert!(matches!(&actions[2], Action::Show { .. }));
        assert!(
            matches!(&actions[3], Action::Movie { path, loop_ } if path.ends_with("clip.webm") && *loop_)
        );
    }
}
