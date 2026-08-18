# ADR 0008 — Story-script layer as RON statements (`scenario` crate)

**Status:** Proposed
**Date:** 2026-08-18
**Decision maker:** John D. Pope

## Context

The engine ships rooms, dialog trees, inventory, audio, and saves, but has no
way to author *sequential* story content — Ren'Py-style scenes of
say/scene/play statements with labels, branching, and mid-story saves. This
is required for visual-novel parity (reference corpus: extracted HarmonyHaven
`.rpy` scripts) and for authored (non-LLM) content generally.

## Decision

Add a new crate `crates/scenario` (workspace name `adventure-scenario`) with:

1. A **RON story format** — `assets/stories/<name>.story.ron`: a map of
   labels to ordered statement lists (`Say`, `Scene`, `Menu`, `If`, `Jump`,
   `Call`, `Return`, `Exec`, …).
2. A **`StoryRunner`** state machine shaped like `DialogRunner`: runs
   statements until the next blocking one, driven by
   `advance`/`choose`/`submit_text`. Presentation statements emit `Action`s
   for the engine loop; the runner never touches ECS/render/audio directly.
3. **Rhai stays the only logic language** — conditions (`If`, choice
   gating), effects (`Exec`, choice `effects`), all via the existing
   `ScriptHost` over `Tags` + `VarTable`. The statement list is flow, not logic.

Design doc: [docs/VN_LAYER_DESIGN.md](../VN_LAYER_DESIGN.md). Roadmap slot:
Phase 8B, parallel to 8A (cutscenes/i18n share the transition/timeline work).

## Rationale

- **Consistency.** ADR 0004 (Rhai) and ADR 0005 (RON data-driven) already
  fixed the 80/20 split; stories are exactly the 80% data + 20% Rhai case.
- **The semantics already exist.** `DialogRunner` proves the runner pattern
  (fail-closed conditions, effect firing, history). The scenario layer is
  that pattern lifted from conversation scope to story scope, plus
  presentation statements and a call stack.
- **Saves/replay fall out for free.** A data-only position
  (`label, index, call_stack`) + existing `VarTable`/`Tags`/versioned saves
  means mid-story saves and Ren'Py-style replay labels need no new machinery.
- **Deterministic and testable.** Headless runner, validator-gated assets —
  same properties that make dialog testable today.

## Alternatives considered

- **Pure Rhai scripts as the story format** (Ren'Py is Python + sugar): Rhai
  has no coroutines or resumable execution; blocking (`Say`, `Menu`) would
  require statement-by-statement AST stepping that breaks inside `while`/fn
  bodies, and labels/gotos don't exist. Rejected — also violates the 80/20 split.
- **Extend `DialogTree` with presentation nodes**: conflates conversation
  scope with story scope; no call stack, no persistent store, and every
  non-dialog command would be a special-cased node. Rejected.
- **Build the layer in PresidentialDilema-FastApi (Python) first**: the
  FastAPI repo has no deterministic runtime, no renderer, and its four
  content models (PD classic, SimWorld, Worldmodel, Worldcraft) have no
  unified schema — we'd build a third interpreter there and port later.
  Rejected; FastAPI instead becomes the CMS/LLM-drafting front-end over the
  same serde schema (see VN_LAYER_DESIGN.md, Bridge section).
- **Rollback/time-travel scrollback (full Ren'Py parity item)**: explicitly
  out of scope for v1 — saves + seen-history cover the practical cases.

## Consequences

- **Positive:** authored VN-content parity with a closed, validatable
  statement set; save slots, seen-skip, replay gallery, and name input
  (`[kevinname]` pattern) all land with it.
- **Positive:** LLM story drafts from the FastAPI side get a machine
  acceptance gate — the validator — before touching the asset tree.
- **Negative:** one more crate to maintain; statement set must stay small
  and disciplined (each addition is a schema/custom-version bump).
- **Neutral:** text markup (`{i}…{/i}`, `[var]`) moves into the `ui` crate as
  a span parser; char registry becomes another RON asset type.
- **Neutral:** `Action` enum in `crates/scripting` gains presentation
  variants (`ShowSprite`, `HideSprite`, `StopMusic`, …).

## References

- `docs/VN_LAYER_DESIGN.md` — full design, phasing (8B.1–8B.4), parity table
- `docs/DECISIONS/0004-rhai-over-lua.md`, `docs/DECISIONS/0005-ron-for-data-driven.md`
- `crates/dialogue/src/runner.rs` — the runner pattern being lifted
- HarmonyHaven extracted scripts (Ren'Py reference corpus)
