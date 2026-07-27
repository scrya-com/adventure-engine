# Scripting — Rhai + Data-Driven Split

> Deep dive on `crates/scripting/` and the scripting philosophy. See `docs/DESIGN.md` for the high-level rationale.

## UE 5.8 reference

Unreal's scripting story is the Blueprint VM:
- `enum EExprToken` (CoreUObject/Public/UObject/Script.h:193) — bytecode opcodes (`EX_Return=0x04`, `EX_Jump=0x06`, `EX_JumpIfNot=0x07`, `EX_LetBool=0x14`, `EX_CallFunction`, `EX_IntConst=0x1D`).
- `UObject::ProcessInternal` (ScriptCore.cpp:1364) — the giant `switch` over `EExprToken` that IS the interpreter (the Kismet VM).
- `FFrame` (Stack.h) — per-call code pointer + locals + registers.
- Hot reload via `HotReload.cpp` + LiveCoding — recompiles DLL, walks every loaded instance, mutates CDO pointers in place.

**We skip all of this.** The Blueprint VM exists to let non-programmers author in-editor graphs. We don't have an editor (we have `tools/inspector` for RON), and our authors are designers writing RON files in their text editor of choice.

## The 80/20 split

**Firm recommendation:**
- **80% pure data-driven** — RON files describing click regions, dialog trees, item interactions, scene transitions. No VM, no GC. Hot-reloaded via `notify` file-watch.
- **20% Rhai** — for arithmetic / conditions (`if score > 5 and has_tag("State.NPC.Bob.Met")`). Rust-native, sandboxed, no `unsafe`, no GC, no FFI friction.

**Skip:**
- **Lua** — FFI surface, GC pauses, two-type-system friction.
- **Custom bytecode VM** — too much engineering for the value.
- **Embedded Python** — heavy dep, GIL, security surface.
- **WASM** — overkill; sandboxing concerns handled by Rhai.

## Why Rhai

- Pure Rust (`#![no_std]`-able).
- Sandboxed by default — no file or network access from scripts.
- No GC — reference counting.
- Designed for embedding in Rust apps; call Rust functions directly from script.
- ~200KB binary size impact.
- Mature, used in production by multiple game engines.
- Syntax close to JS / Lua — easy for designers.

## What Rhai evaluates

```rhai
// Conditions (boolean expression, evaluated to true/false)
score > 5 && has_tag("State.NPC.Bob.Met")

// Side effects (statement list, executed for effect)
set_var("met_bob", true);
give_item("key_cellar");
play_sound("sfx/door_unlock");
```

Rust bindings exposed to Rhai:

```rust
// In crates/scripting/src/bindings.rs
engine.register_fn("has_tag", |state: &mut ScriptState, tag: &str| -> bool {
    state.tags.has(Tag::from(tag))
});

engine.register_fn("set_var", |state: &mut ScriptState, key: &str, val: Dynamic| {
    state.vars.set(key, val);
});

engine.register_fn("give_item", |state: &mut ScriptState, item: &str| {
    state.queue.push(Action::GiveItem(AssetId::from(item)));
});

engine.register_fn("play_sound", |state: &mut ScriptState, sound: &str| {
    state.queue.push(Action::PlaySound(AssetId::from(sound)));
});
```

The script doesn't execute the action directly — it pushes `Action` records into a queue. The engine's main loop drains the queue and dispatches. This keeps Rhai sandboxed and lets us serialize / replay for testing.

## Data-driven RON format

```ron
// assets/scenes/clearing.dialog.ron
DialogTree((
    root: "intro",
    nodes: {
        "intro": DialogNode((
            speaker: "bob",
            text: "Hello there, stranger.",
            choices: [
                Choice((
                    text: "Who are you?",
                    next: "intro_name",
                    // Rhai expression — only show this choice if condition is true
                    when: "has_tag(\"State.Player.KnowsBob\") == false",
                )),
                Choice((
                    text: "Goodbye.",
                    next: "exit",
                )),
            ],
            // Rhai statements — run when this node is entered
            on_enter: [
                "set_var(\"met_bob\", true)",
                "give_tag(\"State.NPC.Bob.Met\")",
            ],
        )),
        "intro_name": DialogNode((
            speaker: "bob",
            text: "Name's Bob. I work the mill.",
            next: "intro",
            on_enter: ["give_tag(\"State.Player.KnowsBob\")"],
        )),
        "exit": DialogNode((
            speaker: "bob",
            text: "Take care.",
            // Terminal node — closes the dialog
        )),
    },
))
```

See `docs/DATA-FORMATS.md` for the full schema.

## Crate layout

```
crates/scripting/
├── Cargo.toml
├── src/
│   ├── lib.rs           # public API: ScriptEngine, ScriptState
│   ├── engine.rs        # Rhai engine wrapper
│   ├── state.rs         # ScriptState: tags + vars + action queue
│   ├── bindings.rs      # register_fn calls
│   └── compile.rs       # AST caching: compile once, eval many
└── tests/
```

## API surface

```rust
pub struct ScriptEngine {
    inner: rhai::Engine,
    ast_cache: HashMap<String, AST>,
}

impl ScriptEngine {
    pub fn new() -> Self { /* register bindings */ }

    pub fn compile(&mut self, name: &str, src: &str) -> Result<(), ScriptError>;
    pub fn eval_condition(&self, name: &str, state: &mut ScriptState) -> Result<bool, ScriptError>;
    pub fn run_statements(&self, name: &str, state: &mut ScriptState) -> Result<(), ScriptError>;
}

pub struct ScriptState<'a> {
    pub tags: &'a mut Tags,
    pub vars: &'a mut VarTable,
    pub queue: Vec<Action>,
}

pub enum Action {
    GiveItem(AssetId),
    RemoveItem(AssetId),
    GiveTag(Tag),
    RemoveTag(Tag),
    PlaySound(AssetId),
    PlayMusic(AssetId),
    StopMusic,
    ChangeScene(AssetId),
    StartDialog(AssetId),
    SetCursor(CursorId),
}
```

## Hot reload

```rust
// In crates/assets/src/watcher.rs
let (tx, rx) = std::sync::mpsc::channel();
let mut watcher = notify::recommended_watcher(tx)?;
watcher.watch("assets/scenes/", RecursiveMode::Recursive)?;

// In main loop
match rx.try_recv() {
    Ok(Event { kind: EventKind::Modify(..), paths, .. }) => {
        for path in paths {
            if path.extension() == Some("ron") {
                assets.reload_scene(&path);
            }
        }
    }
    _ => {}
}
```

Designers save their RON file in their editor → `notify` fires → engine reloads the scene → next frame reflects the change. No recompile, no restart.

## What Rhai scripts CANNOT do

This is by design.

- ❌ Read or write files
- ❌ Access the network
- ❌ Spawn threads
- ❌ Call Rust panics
- ❌ Allocate unbounded memory (Rhai has an `Engine::set_max_string_size` etc.)
- ❌ Loop forever (engine has operation limits)
- ❌ Access raw pointers / `unsafe`

If a script needs to do something on this list, that's an `Action` queued back to the engine. The engine decides whether to honor it.

## Versioning

Rhai ASTs are not stable across Rhai versions. We pin Rhai in `Cargo.toml` and version our script-embedding API via `crates/save/`'s `custom_versions` (see `docs/SAVE.md`). If the binding surface changes (e.g., we rename `give_tag` to `add_tag`), bump the `SCRIPTING_GUID` custom version and migrate.
