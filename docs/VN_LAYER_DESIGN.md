# VN Layer — Ren'Py Parity on Rust + Rhai

> Design for the story-script layer: sequential, authored, cutscene-style content
> running on top of the existing engine crates. Companion ADR:
> [docs/DECISIONS/0008-story-script-layer.md](DECISIONS/0008-story-script-layer.md).

## Why

The engine can already do point-and-click rooms, branching dialog, inventory,
audio, and saves. What it cannot do is **authored sequential storytelling** —
a script that says line A, swaps the background, plays music, branches on a
flag, and resumes from a save slot mid-scene. That is the visual-novel
(Ren'Py) core, and it is the missing half of the product: SCUMM did rooms,
Ren'Py did stories, we want both.

Reference corpus: the extracted HarmonyHaven scripts
(`~/Desktop/HarmonyHaven-extracted/scripts/*.rpy`) — a complete, shippable VN
written entirely in ~40 files of label/menu/scene statements. Every pattern in
this doc maps 1:1 to something in those scripts.

**Verdict on "beeline for parity here vs the FastAPI repo":** build it here.
Ariadne already ships the hard 60% — `ScriptHost` (Rhai conditions/effects
over `Tags` + `VarTable`), `DialogRunner` (branching, fail-closed gating,
history), versioned saves, 4-bus audio, scene transitions. Ren'Py's
menu/if/`$`-variable semantics already exist in `DialogRunner`; they are just
scoped to one conversation instead of a whole story. The FastAPI repo keeps
the services the engine shouldn't have (CMS authoring, LLM generation,
multiplayer) — see [Bridge to PresidentialDilema-FastApi](#bridge) below.

## Parity checklist

| Ren'Py feature | HarmonyHaven usage | Ariadne status | Plan |
|---|---|---|---|
| `label` / `jump` / `call` / `return` | every file | ❌ | `Story` labels + `Jump`/`Call`/`Return` stmts, call stack |
| `say` (character + narration) | everywhere | 🟡 dialog nodes only | `Say`/`Narrate` statements |
| `menu:` with per-choice blocks | every branch point | 🟡 `Choice` in dialog trees | `Menu` statement (reuse `Choice` semantics) |
| `if` / `else` on flags | `if frank_path1 == True:` | 🟡 node-skip only | `If` block statement (Rhai condition) |
| `$ var = …` mutations | `desire += 1` | ✅ Rhai side effects | `Exec` statement (existing `ScriptHost`) |
| `scene bg with Dissolve` | every scene change | 🟡 rooms/hotspots | `Scene` statement + transition enum |
| `show` / `hide` sprite at position | character portraits | 🟡 props | `Show`/`Hide` statements (layer + anchor) |
| `play` / `stop` / `queue` music | per-scene mood | ✅ kira 4-bus | `Play`/`Stop` statements (channel enum) |
| `pause` | beat timing | ❌ | `Pause` statement |
| `renpy.input` (name entry) | `[kevinname]` | ❌ | `Input` statement → var |
| Text markup + `[var]` interpolation | `{i}(...){/i}`, names | ❌ | span parser in `ui` |
| `Character()` registry | characters.rpy | ❌ | `assets/characters/*.char.ron` |
| Save / load mid-story | ✅ Ren'Py | ✅ versioned saves | serialize `StoryPosition` (trivial — it's data) |
| `persistent.*` (cross-save) | name memory, unlocks | ❌ | `persistent.bin` store + unlock tags |
| Seen-text skip / auto-advance | ✅ Ren'Py | ❌ | line-hash seen set in persistent |
| Replay gallery (`renpy.end_replay`) | gallery.rpy | ❌ | `replay` label flag + isolated replay run |
| Screens / minigames (phone texting) | PhoneTexting.rpy | 🟡 immediate-mode `ui` | deferred — RON screens later |
| Rollback (time-travel scrollback) | ✅ Ren'Py | ❌ | **skipped by design** — saves + history instead |

## The format — `assets/stories/<name>.story.ron`

Data-driven, like everything else (ADR 0005). Rhai stays the *only* logic
language (ADR 0004); the statement list is *flow*, not logic.

```ron
(
    id: "ch1_morning",
    entry: "start",
    labels: {
        "start": [
            Scene(( bg: "bg/kitchen_day", with: Dissolve(1.0) )),
            Play(( channel: Music, asset: "music/spark", volume: 0.4 )),
            Narrate("Another day begins. It may seem ordinary, but it isn't."),
            Say(( who: Some("kevin"), text: "Just the usual work stuff." )),
            Exec("set_int(\"day\", 1); add_tag(\"State.Ch1.Started\")"),

            If((
                condition: "has_tag(\"State.Path.Frank\")",
                then: [
                    Say(( who: Some("rachel"), text: "{i}(Why am I thinking about him?){/i}" )),
                ],
                else_: [
                    Narrate("It had been a quiet day."),
                ],
            )),

            Menu((
                prompt: None,
                choices: [
                    (
                        text: "Give in to the temptation.",
                        effects: Some("set_int(\"desire\", desire + 1); set_int(\"loyalty\", loyalty - 1)"),
                        goto: Some("rachel_gives_in"),
                    ),
                    (
                        text: "Resist and focus on something else.",
                        goto: Some("rachel_resists"),
                    ),
                ],
            )),
            Return, // pop call stack (or end if at bottom)
        ],
        "rachel_gives_in": [
            Scene(( bg: "cg/rthm20", with: Dissolve(0.5) )),
            Pause(( seconds: 0.8 )),
            // ...
            Ending(( id: Some("gave_in") )),
        ],
        "rachel_resists": [
            Narrate("She forced herself to focus on the words."),
            Return,
        ],
    },
)
```

### Statement set (v1)

| Statement | Blocking? | Notes |
|---|---|---|
| `Say { who, text }` | **yes** — wait for advance | `who` = char id; `None` = narrator |
| `Narrate(text)` | **yes** | sugar for `Say { who: None }` |
| `Scene { bg, with }` | no | clears sprite layer (Ren'Py semantics) |
| `Show { sprite, at, as, with }` | no | `at` = named anchor (left/center/right/…) |
| `Hide { as, with }` | no | |
| `Play { channel, asset, volume, fade_in, loop, queue }` | no | channel = Music/Sound/Voice/Ambience → kira buses |
| `Stop { channel, fade_out }` | no | |
| `Pause { seconds }` | **yes** | |
| `Exec(rhai)` | no | existing `ScriptHost::run` + Action queue |
| `If { condition, then, else_ }` | — | nested `Vec<Stmt>`; Rhai condition, fail-closed → `else_` |
| `Menu { prompt, choices }` | **yes** — wait for choice | choice: `text` + optional Rhai `condition` (hide) + `effects` + `goto` |
| `Jump(label)` | no | |
| `Call(label)` | no | pushes return position |
| `Return` | no | pops call stack; at bottom = story ends |
| `Input { prompt, var, default }` | **yes** — wait for text | writes `VarTable` string |
| `Ending { id }` | no | marks ending seen (persistent), then `Return`-to-bottom |

Statement set is closed and `#[non_exhaustive]`-free — adding one is a
schema change (bump custom version, see Migration discipline in
[DATA-FORMATS.md](DATA-FORMATS.md)).

## Runner — `crates/scenario`

Same shape as `DialogRunner` (crates/dialogue/src/runner.rs): a small
deterministic state machine the host drives.

```rust
pub struct StoryRunner {
    story_id: String,
    position: StoryPosition,   // (label, stmt index) + call stack
    finished: bool,
    seen: Vec<LabelId>,        // history
}

pub struct StoryPosition {
    label: LabelId,
    index: usize,
    call_stack: Vec<StoryPosition>,  // for Call/Return
}
```

- `start(story, host, vars, tags, persistent)` — enter `entry` label.
- `resume(story, host, …) -> StepResult` — executes statements **until the
  next blocking one**, returning it:

```rust
pub enum StepResult {
    Say { who: Option<CharId>, spans: Vec<Span> },
    Menu { prompt: Option<Vec<Span>>, choices: Vec<VisibleChoice> },
    Input { prompt: Vec<Span>, default: SmolStr },
    Pause { seconds: f32 },
    Finished,
}
```

- `advance()` — player clicked continue (from `Say`/`Pause`).
- `choose(source_index)` — fail-closed condition re-check, fire `effects`,
  then `Jump(goto)` or fall through (mirrors `DialogRunner::choose`
  exactly — including `ChoiceUnavailable` enforcement).
- `submit_text(s)` — from `Input`; strip, empty → `default`, write var.

Key invariant: **the runner never touches render2d/audio/ECS directly.**
Presentation statements emit `Action`s (extend the existing enum in
`crates/scripting`: `SceneChange`, `ShowSprite`, `HideSprite`, `PlayMusic`,
… — several already exist). The engine's main loop drains the queue, exactly
like dialog side effects today. This keeps the crate headless-testable and
the runner pure data-in/data-out.

### Determinism → saves and replay are free

Because position is `(label, index, call_stack)` plus the already-versioned
`VarTable`/`Tags`, a mid-story save is ~50 bytes of extra state on top of the
existing save schema (one new custom version GUID: `STORY_POSITION`).

**Replay** (the Ren'Py gallery pattern): labels may declare
`replay: Some(title)`. The host enumerates replay labels whose unlock
condition holds (persistent tag `Replay.<story>.<label>` — set by `Ending`
or `Exec`). A replay run is: clone `persistent` (not `vars`/`tags`), set a
`is_replay` flag on the runner, `start_at(label)`, and on `Return`-to-bottom
end replay and restore. No special `end_replay()` call needed — `Return`
is it.

## Characters, text markup, interpolation

`assets/characters/<id>.char.ron`:

```ron
(
    id: "rachel",
    name: "rachelname",        // VarTable key resolved at display time
    color: Some((0.92, 0.62, 0.78)),
    prefix: None, suffix: None,
)
```

`ui` gains a span parser (one pass, no regex):
- `{i}…{/i}`, `{b}…{/b}`, `{color=#hex}…{/color}` → style spans
- `[varname]` → interpolation from `VarTable` at render time
  (the `[kevinname]` pattern)
- `~` soft break, `\n` hard break — enough for v1

Validation: markup tags balanced; `[var]` keys exist in `VarTable` defaults
or are set by story `Exec`; char ids referenced by `Say` resolve.

## Persistent store

`persistent.bin` — same versioned envelope as saves, but install-scoped
(Ren'Py `persistent.*`):

- var defaults mirrored from story declarations (`desire`, `loyalty`, …)
- seen-line hashes (for skip-seen) — bounded LRU set
- unlock tags (`Replay.*`, `Ending.*`)

`ScriptHost` gains `get_persistent`/`set_persistent` bindings, namespaced
`"pers.key"` to keep them distinct from save-scoped vars.

## What we deliberately skip (v1)

- **Rollback / scrollback time-travel.** Ren'Py's killer feature is also its
  most complex (log + state rewind through every mutation). Our saves +
  `seen` history cover the practical cases. Revisit only if players ask.
- **ATL / transform animation language.** The phase-8 cutscene timeline
  covers choreography; statements get a fixed anchor set, not a transform DSL.
- **Screen language / minigames** (phone texting). Deferred: the immediate-mode
  `ui` crate can host bespoke screens in Rust per-game. A RON screen format is
  a future ADR if a content-driven need appears.
- **`while` loops in story flow.** Rhai `Exec` covers iteration needs; loops
  in the statement list complicate position serialization for no real gain.

## Phasing

Insert as **Phase 8B** (parallel to 8A cutscene/i18n — they share the
timeline/transition work):

| Sub-phase | Delivers | Exit criteria |
|---|---|---|
| 8B.1 | `scenario` crate: `Say/Narrate/Scene/Play/Stop/Pause/Exec/Jump/If/Menu/Call/Return` + `StoryRunner` + validation | `example-10-vn-story`: a 3-label story with a flag branch and a menu, headless runner tests |
| 8B.2 | char registry, span markup + `[var]` interpolation, `Input` | name-entry flow writing `VarTable`, rendered with substitution |
| 8B.3 | `StoryPosition` in saves (custom version bump) + mid-story save/load | save during `Menu`, restore, choose — same outcome |
| 8B.4 | persistent store, seen-skip, replay labels + gallery enumeration | replay a label in isolation; unlock tag persists across saves |

Content milestone: port one HarmonyHaven scene (e.g. `rachel_returns_home`)
to `.story.ron` as the acceptance test — it exercises every v1 statement.

## Bridge to PresidentialDilema-FastApi

The engine stays pure (no HTTP — repo rule). The FastAPI repo owns authoring
and generation:

1. **CMS authoring** — stories are serde types; `ron` ↔ `json` is free.
   The Worldcraft/Worldmodel editors author JSON; a small exporter emits
   `.story.ron` into the asset tree.
2. **LLM drafting** — the narrator/story-director agents draft stories as
   JSON conforming to the same schema; the `crates/assets` validator runs
   as the acceptance gate (all Rhai compiles, all labels resolve) before a
   draft enters the asset tree. Rust remains the only authority on validity.
3. **Serving** — if Flutter needs to render stories (platform-web), follow
   the `workflow_graph` precedent: Rust parses/serves the compiled story,
   Flutter renders. The runner itself stays embeddable, not networked.

## Naming conventions (additions)

- Story files: `assets/stories/<name>.story.ron`, ids `ch1_morning`.
- Labels: snake_case, unique per story (dialog-node convention).
- Replay unlock tags: `Replay.<story>.<label>`; endings: `Ending.<story>.<id>`.
