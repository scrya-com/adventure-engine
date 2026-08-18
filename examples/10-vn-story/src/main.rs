//! Headless 3-label VN story — Phase 8B.1 exit criterion.
//!
//! Exercises `Scene`, `Play`, `Menu`, `If`, and `Jump` through
//! [`adventure_scenario::StoryRunner`]. No window: same `--headless`
//! pattern as `example-08-shawshank-pac`.
//!
//! ```text
//! cargo test -p adventure-scenario
//! cargo test -p example-10-vn-story
//! cargo run -p example-10-vn-story -- --headless
//! ```

use adventure_scenario::{Action, StepResult, Story, StoryRunner};
use adventure_scripting::ScriptHost;
use adventure_state::{Tag, Tags, VarTable};

/// Three labels: `start` (scene/play/if/menu), `left` (jump target), `right`.
const STORY: &str = r#"
(
    id: "vn_demo",
    entry: "start",
    labels: {
        "start": [
            Scene(( bg: "bg/kitchen_day", with: Dissolve(1.0) )),
            Play(( channel: Music, asset: "music/spark", volume: 0.4, loop_: true )),
            Narrate("Another day begins."),
            If((
                condition: "has_tag(\"State.Path.Frank\")",
                then: [ Narrate("She thought of him again.") ],
                else_: [ Narrate("It had been a quiet morning.") ],
            )),
            Menu((
                prompt: Some("What now?"),
                choices: [
                    ( text: "Walk to the kitchen.", goto: Some("left") ),
                    ( text: "Stay put.", goto: Some("right") ),
                ],
            )),
        ],
        "left": [
            Scene(( bg: "bg/kitchen_night" )),
            Narrate("The kettle clicked off."),
            Jump("right"),
        ],
        "right": [
            Narrate("She sat with the silence."),
            Ending(( id: Some("quiet_morning") )),
        ],
    },
)
"#;

fn run_story(choose_left: bool) -> Result<(Option<String>, Vec<Action>, Vec<String>), String> {
    let story = Story::from_ron_str(STORY).map_err(|e| e.to_string())?;
    let mut runner = StoryRunner::new(&story).map_err(|e| e.to_string())?;
    let host = ScriptHost::new();
    let mut vars = VarTable::new();
    let mut tags = Tags::new();
    if !choose_left {
        tags.add(Tag::new("State.Path.Frank").map_err(|e| e.to_string())?);
    }

    let mut step = runner
        .start(&host, &mut vars, &mut tags)
        .map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    loop {
        match step {
            StepResult::Say { who, text } => {
                let prefix = who.as_deref().map(|w| format!("{w}: ")).unwrap_or_default();
                lines.push(format!("{prefix}{text}"));
                step = runner
                    .advance(&host, &mut vars, &mut tags)
                    .map_err(|e| e.to_string())?;
            }
            StepResult::Menu { choices, .. } => {
                if choices.is_empty() {
                    return Err("menu had no visible choices".into());
                }
                let idx = if choose_left { 0 } else { 1 };
                step = runner
                    .choose(choices[idx].source_index, &host, &mut vars, &mut tags)
                    .map_err(|e| e.to_string())?;
            }
            StepResult::Pause { .. } => {
                step = runner
                    .advance(&host, &mut vars, &mut tags)
                    .map_err(|e| e.to_string())?;
            }
            StepResult::Input { default, .. } => {
                step = runner
                    .submit_text(default.as_str(), &host, &mut vars, &mut tags)
                    .map_err(|e| e.to_string())?;
            }
            StepResult::Finished { ending } => {
                let actions = runner.drain_actions();
                return Ok((ending.map(|s| s.to_string()), actions, lines));
            }
        }
    }
}

fn main() {
    match run_story(true) {
        Ok((ending, actions, lines)) => {
            println!("ending={ending:?}");
            println!("actions={actions:?}");
            for line in &lines {
                println!("  {line}");
            }
            assert!(
                actions.iter().any(|a| matches!(a, Action::Scene { .. })),
                "story must emit Scene"
            );
            assert!(
                actions.iter().any(|a| matches!(a, Action::Play { .. })),
                "story must emit Play"
            );
            println!("vn-story OK ({} lines)", lines.len());
        }
        Err(e) => {
            eprintln!("vn-story FAILED: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_label_left_path() {
        let (ending, actions, lines) = run_story(true).expect("left path");
        assert_eq!(ending.as_deref(), Some("quiet_morning"));
        assert!(actions.iter().any(|a| matches!(a, Action::Scene { .. })));
        assert!(actions.iter().any(|a| matches!(a, Action::Play { .. })));
        assert!(lines.iter().any(|l| l.contains("quiet morning")));
        assert!(lines.iter().any(|l| l.contains("kettle")));
    }

    #[test]
    fn three_label_if_branch() {
        let (ending, _, lines) = run_story(false).expect("frank path");
        assert_eq!(ending.as_deref(), Some("quiet_morning"));
        assert!(lines.iter().any(|l| l.contains("thought of him")));
        assert!(!lines.iter().any(|l| l.contains("kettle")));
    }
}
