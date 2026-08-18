//! Compile + validate: lower a [`Story`] into a flat instruction list.
//!
//! Nested `If` blocks are lowered to `JumpIfFalse` / `Jump` pairs so the
//! runtime position stays a flat `(ip, call_stack)` — pure data, trivially
//! serializable for saves (design: docs/VN_LAYER_DESIGN.md §Runner).
//! Labels become instruction offsets. Every label body ends with an implicit
//! `Return` (falling off the end of a label returns, Ren'Py-call semantics).

use std::collections::BTreeMap;

use adventure_core::SmolStr;

use crate::stmt::{Stmt, Story};

/// Placeholder for jump targets patched during lowering.
const UNRESOLVED: u32 = u32::MAX;

/// Compiled story program (internal to the crate).
#[derive(Clone, Debug)]
pub(crate) struct Compiled {
    /// Flat instruction list.
    pub(crate) instrs: Vec<Ir>,
    /// Label name → instruction offset.
    pub(crate) labels: BTreeMap<SmolStr, u32>,
    /// Instruction offset → label name (for history recording).
    pub(crate) label_offsets: BTreeMap<u32, SmolStr>,
}

/// One lowered instruction.
#[derive(Clone, Debug)]
pub(crate) enum Ir {
    /// Blocking: dialogue line.
    Say {
        /// Speaker id (`None` = narrator).
        who: Option<SmolStr>,
        /// Line text.
        text: SmolStr,
    },
    /// Presentation: background change.
    Scene(crate::stmt::SceneSpec),
    /// Presentation: show sprite.
    Show(crate::stmt::ShowSpec),
    /// Presentation: hide sprite.
    Hide(crate::stmt::HideSpec),
    /// Presentation: start audio.
    Play(crate::stmt::PlaySpec),
    /// Presentation: stop audio.
    Stop(crate::stmt::StopSpec),
    /// Blocking: timed wait.
    Pause {
        /// Seconds.
        seconds: f32,
    },
    /// Rhai side effects.
    Exec(String),
    /// Transfer if the Rhai condition is false.
    JumpIfFalse {
        /// Rhai expression.
        cond: String,
        /// Instruction offset.
        target: u32,
    },
    /// Unconditional transfer.
    Jump(u32),
    /// Call a label, pushing the return address.
    Call(u32),
    /// Blocking data: menu of choices.
    Menu(crate::stmt::MenuSpec),
    /// Pop the call stack; at the bottom, finish.
    Return,
    /// Finish with a named ending (clears the call stack).
    Ending {
        /// Ending id.
        id: Option<SmolStr>,
    },
    /// Blocking: `renpy.input` name entry.
    Input(crate::stmt::InputSpec),
}

/// Compile + validate a story. Returns all validation issues at once
/// (missing labels, Rhai that doesn't parse) so authors fix in one pass.
///
/// # Errors
///
/// Returns [`crate::ScenarioError::Validation`] with every issue found.
pub(crate) fn compile(story: &Story) -> Result<Compiled, crate::ScenarioError> {
    let mut instrs: Vec<Ir> = Vec::new();
    let mut labels: BTreeMap<SmolStr, u32> = BTreeMap::new();
    let mut fixups: Vec<(usize, SmolStr)> = Vec::new();
    let mut issues: Vec<String> = Vec::new();

    for (name, stmts) in &story.labels {
        labels.insert(name.clone(), instrs.len() as u32);
        lower_block(stmts, &mut instrs, &mut fixups);
        instrs.push(Ir::Return);
    }

    // Resolve jump/call fixups against label offsets.
    for (index, label) in &fixups {
        match labels.get(label) {
            Some(&offset) => match &mut instrs[*index] {
                Ir::Jump(target) | Ir::Call(target) => *target = offset,
                _ => unreachable!("fixup recorded on a jump/call instruction"),
            },
            None => issues.push(format!("unknown label `{label}` (jump/call)")),
        }
    }

    // Menu gotos are resolved at runtime — validate them here.
    // Also syntax-check every Rhai snippet (compile-only; no evaluation).
    let engine = rhai::Engine::new();
    for (label, stmts) in &story.labels {
        check_stmts(stmts, &engine, &labels, label, &mut issues);
    }

    if !labels.contains_key(&story.entry) {
        issues.push(format!("entry label `{}` does not exist", story.entry));
    }

    if !issues.is_empty() {
        return Err(crate::ScenarioError::Validation(issues));
    }

    let label_offsets = labels
        .iter()
        .map(|(name, &offset)| (offset, name.clone()))
        .collect();
    Ok(Compiled {
        instrs,
        labels,
        label_offsets,
    })
}

/// Recursively lower authored statements, recording jump/call fixups.
fn lower_block(stmts: &[Stmt], out: &mut Vec<Ir>, fixups: &mut Vec<(usize, SmolStr)>) {
    for stmt in stmts {
        match stmt {
            Stmt::Say(spec) => out.push(Ir::Say {
                who: spec.who.clone(),
                text: spec.text.clone(),
            }),
            Stmt::Narrate(text) => out.push(Ir::Say {
                who: None,
                text: text.clone(),
            }),
            Stmt::Scene(spec) => out.push(Ir::Scene(spec.clone())),
            Stmt::Show(spec) => out.push(Ir::Show(spec.clone())),
            Stmt::Hide(spec) => out.push(Ir::Hide(spec.clone())),
            Stmt::Play(spec) => out.push(Ir::Play(spec.clone())),
            Stmt::Stop(spec) => out.push(Ir::Stop(spec.clone())),
            Stmt::Pause(spec) => out.push(Ir::Pause {
                seconds: spec.seconds,
            }),
            Stmt::Exec(src) => out.push(Ir::Exec(src.clone())),
            Stmt::If(spec) => {
                let jif_index = out.len();
                out.push(Ir::JumpIfFalse {
                    cond: spec.condition.clone(),
                    target: UNRESOLVED,
                });
                lower_block(&spec.then, out, fixups);
                match &spec.else_ {
                    Some(else_block) => {
                        let jump_end_index = out.len();
                        out.push(Ir::Jump(UNRESOLVED));
                        let else_start = out.len() as u32;
                        lower_block(else_block, out, fixups);
                        let end = out.len() as u32;
                        patch_target(out, jif_index, else_start);
                        patch_target(out, jump_end_index, end);
                    }
                    None => {
                        let end = out.len() as u32;
                        patch_target(out, jif_index, end);
                    }
                }
            }
            Stmt::Menu(spec) => out.push(Ir::Menu(spec.clone())),
            Stmt::Jump(label) => {
                fixups.push((out.len(), label.clone()));
                out.push(Ir::Jump(UNRESOLVED));
            }
            Stmt::Call(label) => {
                fixups.push((out.len(), label.clone()));
                out.push(Ir::Call(UNRESOLVED));
            }
            Stmt::Return => out.push(Ir::Return),
            Stmt::Ending(spec) => out.push(Ir::Ending {
                id: spec.id.clone(),
            }),
            Stmt::Input(spec) => out.push(Ir::Input(spec.clone())),
        }
    }
}

/// Patch the target of the jump-like instruction at `index`.
fn patch_target(out: &mut [Ir], index: usize, target: u32) {
    match &mut out[index] {
        Ir::Jump(t) | Ir::Call(t) | Ir::JumpIfFalse { target: t, .. } => *t = target,
        _ => unreachable!("patch recorded on a jump-like instruction"),
    }
}

/// Walk statements: validate menu gotos + Rhai snippets (syntax only).
fn check_stmts(
    stmts: &[Stmt],
    engine: &rhai::Engine,
    labels: &BTreeMap<SmolStr, u32>,
    label: &str,
    issues: &mut Vec<String>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Exec(src) => check_rhai(engine, src, "Exec", label, issues),
            Stmt::If(spec) => {
                check_rhai(engine, &spec.condition, "If condition", label, issues);
                check_stmts(&spec.then, engine, labels, label, issues);
                if let Some(else_block) = &spec.else_ {
                    check_stmts(else_block, engine, labels, label, issues);
                }
            }
            Stmt::Menu(spec) => {
                for (i, choice) in spec.choices.iter().enumerate() {
                    if let Some(cond) = &choice.condition {
                        check_rhai(
                            engine,
                            cond,
                            &format!("Menu choice {i} condition"),
                            label,
                            issues,
                        );
                    }
                    if let Some(effects) = &choice.effects {
                        check_rhai(
                            engine,
                            effects,
                            &format!("Menu choice {i} effects"),
                            label,
                            issues,
                        );
                    }
                    if let Some(goto) = &choice.goto {
                        if !labels.contains_key(goto) {
                            issues.push(format!(
                                "label `{label}`: menu choice {i} goto `{goto}` does not exist"
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Syntax-check one Rhai snippet, recording a friendly issue on failure.
fn check_rhai(engine: &rhai::Engine, src: &str, what: &str, label: &str, issues: &mut Vec<String>) {
    if let Err(err) = engine.compile(src) {
        issues.push(format!("label `{label}`: {what} does not compile: {err}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stmt::{EndingSpec, MenuSpec, Stmt, StoryChoice};

    fn story_with_labels(labels: Vec<(&str, Vec<Stmt>)>) -> Story {
        let story_labels = labels
            .into_iter()
            .map(|(name, stmts)| (SmolStr::new(name), stmts))
            .collect();
        Story {
            id: "t".into(),
            entry: "a".into(),
            labels: story_labels,
        }
    }

    #[test]
    fn missing_entry_fails_validation() {
        let mut story = story_with_labels(vec![("a", vec![Stmt::Return])]);
        story.entry = "nope".into();
        let err = compile(&story).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("entry label"), "{msg}");
    }

    #[test]
    fn dangling_jump_fails_validation() {
        let story = story_with_labels(vec![("a", vec![Stmt::Jump("ghost".into())])]);
        let err = compile(&story).unwrap_err();
        assert!(err.to_string().contains("unknown label `ghost`"));
    }

    #[test]
    fn dangling_menu_goto_fails_validation() {
        let story = story_with_labels(vec![(
            "a",
            vec![Stmt::Menu(MenuSpec {
                prompt: None,
                choices: vec![StoryChoice {
                    text: "x".into(),
                    condition: None,
                    effects: None,
                    goto: Some("ghost".into()),
                }],
            })],
        )]);
        let err = compile(&story).unwrap_err();
        assert!(err.to_string().contains("goto `ghost`"));
    }

    #[test]
    fn bad_rhai_fails_validation() {
        let story = story_with_labels(vec![("a", vec![Stmt::Exec("this is ( not rhai".into())])]);
        assert!(compile(&story).is_err());
    }

    #[test]
    fn if_lowering_produces_patched_jumps() {
        let story = story_with_labels(vec![(
            "a",
            vec![
                Stmt::If(crate::stmt::IfSpec {
                    condition: "true".into(),
                    then: vec![Stmt::Narrate("t".into())],
                    else_: Some(vec![Stmt::Narrate("e".into())]),
                }),
                Stmt::Ending(EndingSpec { id: None }),
            ],
        )]);
        let compiled = compile(&story).unwrap();
        // [0]=JumpIfFalse [1]=Say(then) [2]=Jump(end) [3]=Say(else) [4]=Ending [5]=Return(implicit)
        let kinds: String = compiled
            .instrs
            .iter()
            .map(|i| match i {
                Ir::JumpIfFalse { .. } => 'F',
                Ir::Say { .. } => 'S',
                Ir::Jump(_) => 'J',
                Ir::Return => 'R',
                Ir::Ending { .. } => 'E',
                _ => '?',
            })
            .collect();
        assert_eq!(kinds, "FSJSER");
        // JumpIfFalse target = else-branch start (index 3).
        match &compiled.instrs[0] {
            Ir::JumpIfFalse { target, .. } => assert_eq!(*target, 3),
            other => panic!("expected JumpIfFalse, got {other:?}"),
        }
        // Jump target = after the else block (index 4, the Ending).
        match &compiled.instrs[2] {
            Ir::Jump(target) => assert_eq!(*target, 4),
            other => panic!("expected Jump, got {other:?}"),
        }
    }
}
