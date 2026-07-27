# adventure-engine — Claude Guide

This is a Rust workspace for a point-and-click adventure engine. See `AGENTS.md` for general guidance.

## Claude-specific notes

- **Read `docs/DESIGN.md` first** — it's the synthesis of the UE 5.8 analysis that informed every architectural choice. Questions about "why X" are answered there.
- **Read `docs/DECISIONS/*.md`** before proposing changes to the foundation. Each ADR explains why a choice was made; the *why* matters for edge cases.
- **`crates/locomotion/` is forked code** — preserve its test invariants. The SE→NW moonwalk regression test (`plant.rs`) is load-bearing.
- **`crates/graph/` is vendored** — keep it byte-equivalent to upstream unless we've explicitly decided to diverge.
- **RON is the data format**, not JSON or YAML. Use the `ron` crate.
- **Rhai is the scripting language**, not Lua. Scripts are evaluated via `crates/scripting/`.
- **No global state.** Use dependency injection through `engine::App` resources.
- **No unsafe** outside of `crates/render2d/` (which is wgpu-bound and may need small unsafe blocks for FFI).

## When asked to add a feature

1. Check `docs/ROADMAP.md` to see which phase the feature belongs to.
2. Read the relevant design doc (`docs/RENDERING.md`, `docs/SCRIPTING.md`, etc.).
3. If the change affects the foundation (data format, crate boundary, scripting story), write an ADR in `docs/DECISIONS/`.
4. Implement behind the appropriate crate. Don't add new crates without an ADR.
5. Add or update tests inline (`#[cfg(test)] mod tests`).
6. Run `cargo test --workspace` and `cargo build --workspace` before reporting done.

## When asked to fix a bug

1. Reproduce with a test first.
2. Find root cause — don't paper over with `unwrap()` or `panic!()`.
3. The fix should be small and targeted. No drive-by refactors.

## What NOT to do

- Don't add Lua, Python, or a custom bytecode VM.
- Don't add a HTTP server. AVAL stays separate; FastApi lives in `PresidentialDilema-FastApi`.
- Don't introduce Bevy's umbrella crate. We use `bevy_ecs` directly, not `bevy`.
- Don't add new core dependencies without an ADR justifying them.
- Don't remove `#![deny(missing_docs)]`.
- Don't modify `Cargo.lock` by hand — let cargo update it.

## Commit cadence

User explicitly asked to "commit regularly." Each phase produces multiple commits. See `docs/ROADMAP.md` for the commit map.
