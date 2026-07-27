# ADR 0001 — Fork scene-engine rather than depend on it

**Status:** Accepted
**Date:** 2026-07-27
**Decision maker:** John D. Pope

## Context

The existing `scene-engine` crate at `~/Documents/GitHub/PresidentialDilema-FastApi/scene-engine` is a 3385-LOC headless locomotion SDK covering walk graphs, plant FSM, retargeting, ring compass, and verbs. It is:
- MIT licensed, owned by John D. Pope
- Tagged as a "Dart parity port" with `crm-flutter/lib/scene/` declared the source-of-truth until the Rust API locks
- Well-tested (inline `mod tests` per file, SE→NW moonwalk regression at plant.rs:367)
- Zero-dep except serde + serde_json

The adventure engine needs this locomotion code. Three options:

1. **Path-depend on `scene-engine` as-is.** Add `scene-engine = { path = "../scene-engine" }` to the workspace. scene-engine stays in PresidentialDilema-FastApi.
2. **Fork into `crates/locomotion/`.** Copy the code, rename the package, break the Dart SoT link. Own the result.
3. **Path-depend + contribute back.** Treat PresidentialDilema-FastApi as upstream; submit PRs when we fix bugs.

## Decision

**Fork.** Copy scene-engine's 8 source files + fixtures into `crates/locomotion/`, rename the package, and update docstrings to remove the Dart SoT reference.

## Rationale

1. **The Dart parity link is unwanted weight.** scene-engine explicitly documents that crm-flutter is the source of truth and the Rust port may churn in response. This engine is not a Dart port; inheriting that contract creates false expectations and limits evolution.
2. **Single repo ownership.** Mixing a path-dep into a sibling project's tree makes `cargo` workspaces awkward and breaks if the sibling moves. The engine's `Cargo.toml` shouldn't reference repos we don't control.
3. **The locomotion surface is small enough to fork.** 3385 LOC + tests, no platform-specific code, no build scripts. Fork cost is negligible; ongoing merge cost is also negligible (we're not expecting frequent upstream changes — the SDK is feature-complete for our needs).
4. **Forking preserves attribution.** `Cargo.toml` description and crate docs cite the heritage. License (MIT) is preserved. We're not claiming we wrote it; we're claiming we own this particular fork.
5. **Option 3 (contribute back) is still available post-fork.** If we fix a bug in our fork, we can submit the patch upstream to PresidentialDilema-FastApi. The fork doesn't prevent cooperation; it removes the dep link.

## Consequences

- **Positive:** Engine is self-contained. Crate naming is consistent (`locomotion` not `scene-engine`). Docstrings describe our engine, not a Dart port. We can rename types if needed (e.g., drop the `Scrya` references).
- **Negative:** Bug fixes downstream of scene-engine must be applied manually to both repos. We accept this risk — scene-engine is in maintenance mode for crm-flutter, not active feature work.
- **Neutral:** License file in `crates/locomotion/` notes the heritage (John D. Pope, MIT).

## Alternatives considered

**Path-depend** (Option 1): Rejected because it forces every contributor to clone PresidentialDilema-FastApi alongside adventure-engine, complicates CI, and entangles two projects that have different release cadences.

**Contribute back** (Option 3): Rejected as a primary strategy because the Dart SoT contract makes scene-engine a moving target. We can still contribute patches opportunistically.

## References

- scene-engine `AGENTS.md:9` — Dart SoT declaration
- scene-engine `Cargo.toml:5` — `description = "Headless Scrya Scene adventure engine — Dart parity port"`
- `docs/DESIGN.md` — Heritage integration section
