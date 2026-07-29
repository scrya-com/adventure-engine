//! `ScriptHost` — Rhai engine wrapper for dialog conditions + side effects.
//!
//! Adventure scripts are tiny — they read tags / vars to gate choices,
//! and they write tags / vars as side effects. We expose:
//!   * `has_tag("State.NPC.Bob.Met")` → bool
//!   * `has_any_tag("State.NPC.Bob")` → bool (hierarchical parent match)
//!   * `add_tag("...")` / `remove_tag("...")` → ()
//!   * `set_int(name, val)`, `set_float`, `set_bool`, `set_str`
//!   * Plain variable references via scope (e.g. `met_count >= 1`)
//!
//! Conditions evaluate to a bool. Side effects are a sequence of
//! statements that mutate state via the registered helpers. Side-effect
//! scripts get the **same** read helpers + var scope as conditions, so
//! `if has_tag(...) { set_int(...) }` works.
//!
//! Each call builds a sandboxed engine with op limits (ADR sandboxing).
//! Registration helpers are shared between `eval_condition` and `run`.

use std::sync::{Arc, Mutex};

use adventure_state::{Tag, Tags};
use adventure_state::{VarTable, VarValue};

use crate::error::ScriptError;

/// Soft sandbox limits for dialog snippets.
const MAX_OPS: u64 = 10_000;
const MAX_EXPR_DEPTH: usize = 32;

/// A Rhai-backed script host. Stateless between calls; cheap to construct.
#[derive(Debug, Default, Clone)]
pub struct ScriptHost;

impl ScriptHost {
    /// Empty host.
    pub fn new() -> Self {
        Self
    }

    fn make_engine() -> rhai::Engine {
        let mut engine = rhai::Engine::new();
        engine.set_max_operations(MAX_OPS);
        engine.set_max_expr_depths(MAX_EXPR_DEPTH, MAX_EXPR_DEPTH);
        engine
    }

    /// Push `VarTable` entries into a Rhai scope for expression evaluation.
    fn push_vars(scope: &mut rhai::Scope<'_>, vars: &VarTable) {
        for (k, v) in vars.iter() {
            match v {
                VarValue::I(n) => {
                    scope.push(k.as_str(), *n as i64);
                }
                VarValue::F(f) => {
                    scope.push(k.as_str(), *f);
                }
                VarValue::B(b) => {
                    scope.push(k.as_str(), *b);
                }
                VarValue::S(s) => {
                    scope.push(k.as_str(), s.as_str().to_string());
                }
                VarValue::Asset(_) => { /* skip — opaque */ }
            }
        }
    }

    /// Register read-only tag helpers that close over a snapshot of tags.
    fn register_tag_reads_snapshot(engine: &mut rhai::Engine, tags: Arc<Tags>) {
        let tags_for_fn = tags.clone();
        engine.register_fn("has_tag", move |s: &str| -> bool {
            Tag::new(s).map(|t| tags_for_fn.has(&t)).unwrap_or(false)
        });
        let tags_for_any = tags;
        engine.register_fn("has_any_tag", move |s: &str| -> bool {
            Tag::new(s)
                .map(|t| tags_for_any.has_any(&t))
                .unwrap_or(false)
        });
    }

    /// Register write helpers for side-effect scripts.
    fn register_writes(
        engine: &mut rhai::Engine,
        vars: Arc<Mutex<VarTable>>,
        tags: Arc<Mutex<Tags>>,
    ) {
        let tags_for_add = tags.clone();
        engine.register_fn("add_tag", move |s: &str| {
            if let Ok(t) = Tag::new(s) {
                tags_for_add.lock().unwrap().add(t);
            }
        });
        let tags_for_remove = tags;
        engine.register_fn("remove_tag", move |s: &str| {
            if let Ok(t) = Tag::new(s) {
                tags_for_remove.lock().unwrap().remove(&t);
            }
        });

        let vars_for_int = vars.clone();
        engine.register_fn("set_int", move |k: &str, v: i64| {
            vars_for_int.lock().unwrap().set(k, VarValue::I(v));
        });
        let vars_for_float = vars.clone();
        engine.register_fn("set_float", move |k: &str, v: f64| {
            vars_for_float.lock().unwrap().set(k, VarValue::F(v));
        });
        let vars_for_bool = vars.clone();
        engine.register_fn("set_bool", move |k: &str, v: bool| {
            vars_for_bool.lock().unwrap().set(k, VarValue::B(v));
        });
        let vars_for_str = vars;
        engine.register_fn("set_str", move |k: &str, v: &str| {
            vars_for_str
                .lock()
                .unwrap()
                .set(k, VarValue::S(adventure_core::SmolStr::new(v)));
        });
    }

    /// Evaluate a Rhai expression against the supplied state. Returns
    /// the bool result.
    pub fn eval_condition(
        &self,
        expr: &str,
        vars: &VarTable,
        tags: &Tags,
    ) -> Result<bool, ScriptError> {
        let expr_trimmed = expr.trim();
        if expr_trimmed.is_empty() {
            return Ok(true);
        }

        let mut engine = Self::make_engine();
        Self::register_tag_reads_snapshot(&mut engine, Arc::new(tags.clone()));

        let mut scope = rhai::Scope::new();
        Self::push_vars(&mut scope, vars);

        let result: rhai::Dynamic = engine
            .eval_expression_with_scope(&mut scope, expr_trimmed)
            .map_err(|e| ScriptError::Rhai(e.to_string()))?;

        let dbg = format!("{result:?}");
        result
            .try_cast::<bool>()
            .ok_or_else(|| ScriptError::NotBool(dbg))
    }

    /// Run a sequence of Rhai statements, mutating the supplied state.
    ///
    /// Side effects happen via registered helpers (`add_tag`,
    /// `remove_tag`, `set_int`, `set_float`, `set_bool`, `set_str`).
    /// Read helpers (`has_tag`, `has_any_tag`) and var scope are also
    /// available so scripts can branch on state.
    pub fn run(
        &self,
        stmts: &str,
        vars: &mut VarTable,
        tags: &mut Tags,
    ) -> Result<(), ScriptError> {
        let stmts_trimmed = stmts.trim();
        if stmts_trimmed.is_empty() {
            return Ok(());
        }

        let vars_arc: Arc<Mutex<VarTable>> = Arc::new(Mutex::new(std::mem::take(vars)));
        let tags_arc: Arc<Mutex<Tags>> = Arc::new(Mutex::new(std::mem::take(tags)));

        let run_result = {
            let mut engine = Self::make_engine();

            // Live-mutex reads so mid-script has_tag sees tags added earlier
            // in the same run.
            let tags_for_has = tags_arc.clone();
            engine.register_fn("has_tag", move |s: &str| -> bool {
                Tag::new(s)
                    .map(|t| tags_for_has.lock().unwrap().has(&t))
                    .unwrap_or(false)
            });
            let tags_for_any = tags_arc.clone();
            engine.register_fn("has_any_tag", move |s: &str| -> bool {
                Tag::new(s)
                    .map(|t| tags_for_any.lock().unwrap().has_any(&t))
                    .unwrap_or(false)
            });

            Self::register_writes(&mut engine, vars_arc.clone(), tags_arc.clone());

            let mut scope = rhai::Scope::new();
            {
                let v = vars_arc.lock().unwrap();
                Self::push_vars(&mut scope, &v);
            }

            engine
                .run_with_scope(&mut scope, stmts_trimmed)
                .map_err(|e| ScriptError::Rhai(e.to_string()))
        };
        run_result?;

        *vars = Arc::try_unwrap(vars_arc)
            .ok()
            .expect("all script closures dropped before this point")
            .into_inner()
            .unwrap();
        *tags = Arc::try_unwrap(tags_arc)
            .ok()
            .expect("all script closures dropped before this point")
            .into_inner()
            .unwrap();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags_with(tag_strs: &[&str]) -> Tags {
        let mut t = Tags::new();
        for &s in tag_strs {
            t.add(Tag::new(s).unwrap());
        }
        t
    }

    #[test]
    fn empty_condition_is_true() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = Tags::new();
        assert!(host.eval_condition("", &v, &t).unwrap());
        assert!(host.eval_condition("   ", &v, &t).unwrap());
    }

    #[test]
    fn literal_true() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = Tags::new();
        assert!(host.eval_condition("true", &v, &t).unwrap());
        assert!(!host.eval_condition("false", &v, &t).unwrap());
    }

    #[test]
    fn has_tag_true_when_present() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = tags_with(&["State.NPC.Bob.Met"]);
        assert!(host
            .eval_condition(r#"has_tag("State.NPC.Bob.Met")"#, &v, &t)
            .unwrap());
    }

    #[test]
    fn has_tag_false_when_absent() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = Tags::new();
        assert!(!host
            .eval_condition(r#"has_tag("State.NPC.Bob.Met")"#, &v, &t)
            .unwrap());
    }

    #[test]
    fn and_or_combinators() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = tags_with(&["State.NPC.Bob.Met", "State.NPC.Sue.Met"]);
        assert!(host
            .eval_condition(
                r#"has_tag("State.NPC.Bob.Met") && has_tag("State.NPC.Sue.Met")"#,
                &v,
                &t
            )
            .unwrap());
        assert!(host
            .eval_condition(
                r#"has_tag("State.NPC.Bob.Met") || has_tag("State.NPC.Missing")"#,
                &v,
                &t
            )
            .unwrap());
        assert!(!host
            .eval_condition(
                r#"has_tag("State.NPC.Bob.Met") && has_tag("State.NPC.Missing")"#,
                &v,
                &t
            )
            .unwrap());
    }

    #[test]
    fn has_any_tag_matches_child() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = tags_with(&["State.NPC.Bob.Met"]);
        assert!(host
            .eval_condition(r#"has_any_tag("State.NPC.Bob")"#, &v, &t)
            .unwrap());
    }

    #[test]
    fn variables_visible_in_scope() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        v.set("met_count", VarValue::I(3));
        let t = Tags::new();
        assert!(host.eval_condition("met_count >= 1", &v, &t).unwrap());
        assert!(!host.eval_condition("met_count < 1", &v, &t).unwrap());
        assert!(host.eval_condition("met_count == 3", &v, &t).unwrap());
    }

    #[test]
    fn non_bool_condition_errors() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = Tags::new();
        let err = host.eval_condition("1 + 1", &v, &t).unwrap_err();
        assert!(matches!(err, ScriptError::NotBool(_)));
    }

    #[test]
    fn run_add_tag_writes_state() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut t = Tags::new();
        host.run(r#"add_tag("State.NPC.Bob.Met")"#, &mut v, &mut t)
            .unwrap();
        assert!(t.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
    }

    #[test]
    fn run_remove_tag_clears_state() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut t = tags_with(&["State.NPC.Bob.Met"]);
        host.run(r#"remove_tag("State.NPC.Bob.Met")"#, &mut v, &mut t)
            .unwrap();
        assert!(!t.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
    }

    #[test]
    fn run_set_int_writes_state() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut t = Tags::new();
        host.run(r#"set_int("score", 42)"#, &mut v, &mut t).unwrap();
        match v.get("score") {
            Some(VarValue::I(42)) => {}
            other => panic!("expected I(42), got {other:?}"),
        }
    }

    #[test]
    fn run_set_str_writes_state() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut t = Tags::new();
        host.run(r#"set_str("greeting", "hello")"#, &mut v, &mut t)
            .unwrap();
        match v.get("greeting") {
            Some(VarValue::S(s)) if s == "hello" => {}
            other => panic!("expected S(hello), got {other:?}"),
        }
    }

    #[test]
    fn run_multiple_statements() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut t = Tags::new();
        let src = r#"
            add_tag("State.NPC.Bob.Met");
            set_int("met_count", 1);
            set_str("last_met", "Bob");
        "#;
        host.run(src, &mut v, &mut t).unwrap();
        assert!(t.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
        match v.get("met_count") {
            Some(VarValue::I(1)) => {}
            other => panic!("met_count wrong: {other:?}"),
        }
        match v.get("last_met") {
            Some(VarValue::S(s)) if s == "Bob" => {}
            other => panic!("last_met wrong: {other:?}"),
        }
    }

    #[test]
    fn run_can_branch_on_has_tag() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        let mut t = tags_with(&["State.NPC.Bob.Met"]);
        host.run(
            r#"if has_tag("State.NPC.Bob.Met") { set_int("ok", 1); }"#,
            &mut v,
            &mut t,
        )
        .unwrap();
        match v.get("ok") {
            Some(VarValue::I(1)) => {}
            other => panic!("expected ok=1, got {other:?}"),
        }
    }

    #[test]
    fn run_can_read_vars_in_scope() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        v.set("score", VarValue::I(5));
        let mut t = Tags::new();
        host.run(r#"set_int("score", score + 1)"#, &mut v, &mut t)
            .unwrap();
        match v.get("score") {
            Some(VarValue::I(6)) => {}
            other => panic!("expected score=6, got {other:?}"),
        }
    }

    #[test]
    fn run_empty_is_no_op() {
        let host = ScriptHost::new();
        let mut v = VarTable::new();
        v.set("foo", VarValue::I(5));
        let mut t = tags_with(&["State.NPC.Bob.Met"]);
        host.run("", &mut v, &mut t).unwrap();
        assert!(matches!(v.get("foo"), Some(VarValue::I(5))));
        assert!(t.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
    }

    #[test]
    fn invalid_expression_errors() {
        let host = ScriptHost::new();
        let v = VarTable::new();
        let t = Tags::new();
        assert!(host.eval_condition("this is not valid", &v, &t).is_err());
    }
}
