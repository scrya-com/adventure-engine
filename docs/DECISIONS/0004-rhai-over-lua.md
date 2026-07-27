# ADR 0004 — Rhai for scripting, not Lua

**Status:** Accepted
**Date:** 2026-07-27
**Decision maker:** John D. Pope

## Context

The engine needs an embedded scripting language for the 20% of adventure logic that can't be expressed in pure data (RON). Use cases:
- Conditions in dialog choices: `if score > 5 and has_tag("State.NPC.Bob.Met")`
- Side effects in dialog nodes: `set_var("met_bob", true); give_item("key_cellar")`
- Combine rules for inventory: `if has_item("oil") and has_item("lamp") then ...`

Options:

1. **`rhai`** — Rust-native scripting language
2. **`mlua` / `rlua`** — Lua bindings for Rust
3. **`boa`** — JavaScript engine in Rust
4. **`deno_core`** — V8-backed JS
5. **Custom bytecode VM** (a la UE Blueprint)
6. **`pyo3`** — embedded Python

## Decision

**Use Rhai.** Skip all alternatives.

## Rationale

1. **Pure Rust, zero `unsafe`.** Rhai is implemented entirely in safe Rust. No FFI surface, no native library deps. Compiles cleanly on every Rust-supported platform.
2. **Sandboxed by default.** No file or network access from scripts. Designers cannot accidentally (or maliciously) read player files. Critical for an engine that ships `.ron` files containing Rhai snippets.
3. **No GC.** Rhai uses reference counting, not tracing GC. No GC pauses during gameplay. Predictable memory behavior.
4. **Designed for embedding.** Rhai's `Engine::register_fn` lets us expose Rust functions directly to scripts:
   ```rust
   engine.register_fn("has_tag", |state: &mut State, t: &str| state.tags.has(t.into()));
   ```
   Lua's equivalent (Lua C API) requires unsafe FFI; Rhai's is safe Rust.
5. **Single type system.** Rhai values map directly to Rust types via `serde`. No two-world impedance mismatch.
6. **Small.** ~200 KB binary size impact. Lua is similar; Python is 30+ MB; V8 is 50+ MB.
7. **Mature.** Used in production by multiple game engines and applications. Documentation is good.
8. **Syntax close to Lua/JS.** Designers coming from either world can read Rhai without much friction.
9. **Operation limits.** Engine has built-in limits (`set_max_operations`, `set_max_string_size`) to prevent runaway scripts from hanging the engine.
10. **AST caching.** Compile once, eval many times. Scripts parsed at asset load; runtime is just eval.

## What Rhai is NOT good at

Be honest about scope:
- **Not for hot inner loops.** Rhai is interpreted; it's not fast enough for per-vertex math or per-frame system logic. Use Rust for those.
- **Not for modding.** Rhai scripts are authored by designers alongside the rest of the asset set; they ship with the game. Players don't write Rhai.
- **Not for save files.** Save data is Rust structs via `rmp-serde`. Rhai is for authored content, not runtime data.

## Consequences

- **Positive:** No FFI surface, no GC pauses, no native deps. Designers get a real scripting language. Engine stays sandboxed.
- **Negative:** Rhai is slower than LuaJIT (when LuaJIT is available — which is not always). For our use case (evaluating conditions on dialog choice selection, running side-effect statements on dialog node entry), this doesn't matter.
- **Negative:** Rhai's ecosystem is smaller than Lua's. We're not relying on community libraries — we write our own bindings.
- **Neutral:** Rhai ASTs are not stable across Rhai versions. We pin Rhai and version our script API via `crates/save/` custom versions.

## Action queue pattern (important)

Rhai scripts don't execute side effects directly. They push `Action` records into a queue:

```rust
fn give_item(state: &mut ScriptState, item: &str) {
    state.queue.push(Action::GiveItem(AssetId::from(item)));
}
```

The engine drains the queue and executes actions in the main loop. This:
- Keeps Rhai pure (no direct world mutation)
- Enables serialization / replay for testing
- Lets us defer actions to a specific tick (e.g., next frame, after dialog closes)

## Alternatives considered

**Lua**: Rejected because of FFI friction (`mlua` requires `unsafe` blocks for some APIs), tracing GC pauses (small but real), and two-type-system friction (Lua tables vs Rust structs).

**JavaScript (boa/deno_core)**: Rejected because JS's async model, prototype inheritance, and large dep surface don't fit our needs.

**Blueprint-style custom VM**: Rejected as too much engineering for the value. UE's `EExprToken` opcodes and `ProcessInternal` switch exist to support visual editing — we don't have an editor.

**Python (pyo3)**: Rejected for binary size, GIL complexity, and security surface (Python has filesystem access by default).

## References

- `docs/SCRIPTING.md` — full design
- `crates/scripting/` — implementation
- CoreUObject/Public/UObject/Script.h:193 (UE Blueprint VM opcodes — what we skip)
