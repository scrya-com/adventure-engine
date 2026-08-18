//! Authored story format — `.story.ron`.
//!
//! A [`Story`] is a map of labels to ordered statement lists. Statements are
//! *flow* (say / show / branch / jump); all *logic* (conditions, effects)
//! stays Rhai via `adventure-scripting` (ADR 0004 / 0005).
//!
//! Parsing uses RON with `ImplicitSome` enabled, so optional fields can be
//! written bare (`with: Dissolve(1.0)` instead of
//! `with: Some(Dissolve(1.0))`). See [`Story::from_ron_str`].

use std::collections::BTreeMap;

use adventure_core::SmolStr;
use serde::{Deserialize, Serialize};

/// A story: entry label + named labels of ordered statements.
///
/// RON:
///
/// ```ron
/// (
///     id: "ch1_morning",
///     entry: "start",
///     labels: {
///         "start": [ Narrate("Hello."), Jump("end") ],
///         "end":   [ Ending((id: "done")) ],
///     },
/// )
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Story {
    /// Story id (for save files / debugging).
    pub id: String,
    /// Label entered by [`crate::StoryRunner::start`].
    pub entry: SmolStr,
    /// Label name → ordered statements. Falling off the end of a label is an
    /// implicit `Return` (pops the call stack; story ends at the bottom).
    pub labels: BTreeMap<SmolStr, Vec<Stmt>>,
}

impl Story {
    /// Parse from RON source with `ImplicitSome` enabled.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ScenarioError::Ron`] on parse failure. Use
    /// [`crate::StoryRunner::new`] for full validation (labels, Rhai).
    pub fn from_ron_str(src: &str) -> Result<Self, crate::ScenarioError> {
        let options = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
        options
            .from_str(src)
            .map_err(|e| crate::ScenarioError::Ron(e.to_string()))
    }

    /// Serialize back to RON (explicit `Some(...)`; still re-parses via
    /// [`Story::from_ron_str`]). Useful for authoring tools / round-trips.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ScenarioError::Serialize`] on failure.
    pub fn to_ron_str(&self) -> Result<String, crate::ScenarioError> {
        let options = ron::Options::default()
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
        options
            .to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| crate::ScenarioError::Serialize(e.to_string()))
    }
}

/// One authored statement inside a label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    /// A dialogue line. Blocking: waits for player advance.
    Say(SaySpec),
    /// Narrator line. Blocking. Sugar for `Say` with no speaker.
    Narrate(SmolStr),
    /// Change the background (clears the sprite layer, Ren'Py semantics).
    Scene(SceneSpec),
    /// Show a sprite / portrait at an anchor.
    Show(ShowSpec),
    /// Hide a shown sprite.
    Hide(HideSpec),
    /// Start playing an asset on a channel.
    Play(PlaySpec),
    /// Stop a channel.
    Stop(StopSpec),
    /// Wait. Blocking.
    Pause(PauseSpec),
    /// Run Rhai side effects against vars / tags.
    Exec(String),
    /// Conditional block over a Rhai condition.
    If(IfSpec),
    /// Present choices. Blocking: waits for player selection.
    Menu(MenuSpec),
    /// Transfer to a label (does not return).
    Jump(SmolStr),
    /// Call a label (returns via `Return` / end-of-label).
    Call(SmolStr),
    /// Pop the call stack; at the bottom, the story ends.
    Return,
    /// End the story with a named ending (clears the call stack).
    Ending(EndingSpec),
    /// Prompt for a string (`renpy.input`) and write it to a var. Blocking.
    Input(InputSpec),
}

/// Speaker + text for [`Stmt::Say`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SaySpec {
    /// Character id, or `None` for the narrator (rare — prefer `Narrate`).
    #[serde(default)]
    pub who: Option<SmolStr>,
    /// The line (markup like `{i}…{/i}` is parsed by the `ui` crate, 8B.2).
    pub text: SmolStr,
}

/// Background change for [`Stmt::Scene`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SceneSpec {
    /// Background asset path.
    pub bg: SmolStr,
    /// Optional transition.
    #[serde(default)]
    pub with: Option<Transition>,
}

/// Sprite show for [`Stmt::Show`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShowSpec {
    /// Sprite asset path.
    pub sprite: SmolStr,
    /// Named screen anchor.
    #[serde(default)]
    pub at: Option<Anchor>,
    /// Tag this sprite hides / replaces (`as` is a Rust keyword).
    #[serde(default)]
    pub as_: Option<SmolStr>,
    /// Optional transition.
    #[serde(default)]
    pub with: Option<Transition>,
}

/// Sprite hide for [`Stmt::Hide`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HideSpec {
    /// Which shown sprite to hide (`None` = all).
    #[serde(default)]
    pub as_: Option<SmolStr>,
    /// Optional transition.
    #[serde(default)]
    pub with: Option<Transition>,
}

/// Audio start for [`Stmt::Play`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaySpec {
    /// Which channel (maps to the audio crate's buses).
    pub channel: Channel,
    /// Audio asset path.
    pub asset: SmolStr,
    /// Linear volume 0..1.
    #[serde(default)]
    pub volume: Option<f32>,
    /// Fade-in seconds.
    #[serde(default)]
    pub fade_in: Option<f32>,
    /// Loop the asset (`loop` is a Rust keyword).
    #[serde(default)]
    pub loop_: bool,
}

/// Audio stop for [`Stmt::Stop`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StopSpec {
    /// Which channel to stop.
    pub channel: Channel,
    /// Fade-out seconds.
    #[serde(default)]
    pub fade_out: Option<f32>,
}

/// Wait for [`Stmt::Pause`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PauseSpec {
    /// Seconds to wait.
    pub seconds: f32,
}

/// Conditional block for [`Stmt::If`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IfSpec {
    /// Rhai expression (evaluates bool; fails closed to `else_`).
    pub condition: String,
    /// Statements when true.
    pub then: Vec<Stmt>,
    /// Statements when false.
    #[serde(default)]
    pub else_: Option<Vec<Stmt>>,
}

/// Choice menu for [`Stmt::Menu`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuSpec {
    /// Optional prompt shown above the choices.
    #[serde(default)]
    pub prompt: Option<SmolStr>,
    /// Choices in authored order; `source_index` refers to this list.
    pub choices: Vec<StoryChoice>,
}

/// One choice inside a [`Stmt::Menu`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryChoice {
    /// Player-facing choice text.
    pub text: SmolStr,
    /// Rhai condition; false (or script error) hides the choice.
    #[serde(default)]
    pub condition: Option<String>,
    /// Rhai side effects fired on selection (before `goto`).
    #[serde(default)]
    pub effects: Option<String>,
    /// Label to jump to; `None` falls through past the menu.
    #[serde(default)]
    pub goto: Option<SmolStr>,
}

/// Text prompt for [`Stmt::Input`] (`renpy.input` / `[kevinname]`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputSpec {
    /// Prompt shown above the field.
    pub prompt: SmolStr,
    /// `VarTable` key to write (`kevinname`, `rachelname`, …).
    pub var: SmolStr,
    /// Used when the player submits an empty string.
    #[serde(default)]
    pub default: Option<SmolStr>,
}

/// Named ending for [`Stmt::Ending`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EndingSpec {
    /// Ending id (recorded on [`crate::StepResult::Finished`]).
    #[serde(default)]
    pub id: Option<SmolStr>,
}

/// Screen transition kinds (v1 fixed set — no ATL-style transform DSL).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Transition {
    /// Hard cut (no fade) — the default when `with` is omitted.
    Cut,
    /// Crossfade over the given seconds.
    Dissolve(f32),
    /// Fade through black over the given seconds.
    Fade(f32),
}

/// Named sprite anchors (v1 fixed set).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Anchor {
    /// Left third of the screen.
    Left,
    /// Horizontally centered.
    Center,
    /// Right third of the screen.
    Right,
}

/// Audio channels (mirror the audio crate's four kira buses).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Channel {
    /// Music bus.
    Music,
    /// Sound-effects bus.
    Sound,
    /// Voice bus.
    Voice,
    /// Ambience bus.
    Ambience,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
(
    id: "sample",
    entry: "start",
    labels: {
        "start": [
            Scene(( bg: "bg/kitchen_day", with: Dissolve(1.0) )),
            Play(( channel: Music, asset: "music/spark", volume: 0.4 )),
            Narrate("Another day begins."),
            Say(( who: Some("kevin"), text: "Just the usual work stuff." )),
            Say(( text: "Bare say — narrator via omitted who." )),
            Exec("set_int(\"day\", 1)"),
            If((
                condition: "has_tag(\"State.Path.Frank\")",
                then: [ Narrate("frank") ],
                else_: [ Narrate("quiet") ],
            )),
            Menu((
                prompt: Some("What now?"),
                choices: [
                    ( text: "Go left", effects: Some("set_int(\"desire\", 1)"), goto: Some("left") ),
                    ( text: "Go right", goto: Some("right") ),
                ],
            )),
        ],
        "left":   [ Pause(( seconds: 0.8 )), Ending(( id: Some("gave_in") )) ],
        "right":  [ Narrate("ok"), Return ],
    },
)
"#;

    #[test]
    fn parses_sample_with_implicit_some() {
        let story = Story::from_ron_str(SAMPLE).unwrap();
        assert_eq!(story.id, "sample");
        assert_eq!(story.entry, "start");
        assert_eq!(story.labels.len(), 3);
        assert_eq!(story.labels["start"].len(), 8);
    }

    #[test]
    fn explicit_some_still_parses() {
        let src = r#"
(
    id: "x",
    entry: "a",
    labels: { "a": [ Show(( sprite: "s/kevin", at: Some(Left), as_: Some("kevin") )) ] },
)
"#;
        let story = Story::from_ron_str(src).unwrap();
        match &story.labels["a"][0] {
            Stmt::Show(spec) => {
                assert_eq!(spec.at, Some(Anchor::Left));
                assert_eq!(spec.as_.as_deref(), Some("kevin"));
            }
            other => panic!("expected Show, got {other:?}"),
        }
    }

    #[test]
    fn defaults_apply_for_omitted_optionals() {
        let src = r#"
(
    id: "x",
    entry: "a",
    labels: { "a": [ Play(( channel: Sound, asset: "sfx/blip" )), Scene(( bg: "b" )) ] },
)
"#;
        let story = Story::from_ron_str(src).unwrap();
        match &story.labels["a"][0] {
            Stmt::Play(spec) => {
                assert_eq!(spec.channel, Channel::Sound);
                assert_eq!(spec.volume, None);
                assert!(!spec.loop_);
            }
            other => panic!("expected Play, got {other:?}"),
        }
        match &story.labels["a"][1] {
            Stmt::Scene(spec) => assert_eq!(spec.with, Some(Transition::Cut).filter(|_| false)),
            other => panic!("expected Scene, got {other:?}"),
        }
    }

    #[test]
    fn ron_roundtrip() {
        let story = Story::from_ron_str(SAMPLE).unwrap();
        let ser = story.to_ron_str().unwrap();
        let back = Story::from_ron_str(&ser).unwrap();
        assert_eq!(story, back);
    }

    #[test]
    fn bad_ron_errors() {
        assert!(Story::from_ron_str("not ron at all").is_err());
    }
}
