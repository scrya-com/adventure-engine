# ADR 0007 — Vendor aval_graph rather than path-depend

**Status:** Accepted
**Date:** 2026-07-27
**Decision maker:** John D. Pope

## Context

`aval_graph` is a 634-LOC pure Rust crate at `~/Documents/GitHub/aval/flutter/rust/aval_graph/`. It implements ring-arc route planning for AVAL's motion state graphs. License: MIT OR Apache-2.0. Zero dependencies.

Options:

1. **Vendor** into `crates/graph/` — copy the source, own it.
2. **Path-depend** — `[dependencies] graph = { path = "../../aval/flutter/rust/aval_graph" }`.
3. **Publish to crates.io** and version-depend — submit upstream PR to publish.
4. **Git submodule** — pull aval_graph in as a submodule.

## Decision

**Vendor.** Copy `aval_graph/src/lib.rs` into `crates/graph/src/lib.rs`, preserve attribution and dual license.

## Rationale

1. **The aval repo is large and unrelated.** Path-depending on a file inside `aval/flutter/rust/aval_graph/` forces contributors to clone the entire AVAL monorepo (TypeScript packages, Flutter ports, examples, certification tooling) just to get one 634-LOC file. That's a 100MB+ clone for ~20KB of code.
2. **AVAL has its own release cadence.** Aval is in active development with breaking changes between versions (see `aval/docs/project/1.0.md`). Path-dep would mean our build breaks when AVAL cuts a release. We want to upgrade on our schedule, not theirs.
3. **aval_graph is stable and small.** The ring-arc math hasn't changed meaningfully in months (last modification Jul 27, but that's a recent refresh — the algorithm itself is locked). 634 LOC is well within "easy to audit and own."
4. **We need to maintain attribution regardless.** Vendoring makes the heritage explicit: the file header says "Pure Rust port of AVAL ring arc + multi-hop route planning," Cargo.toml cites `aval/flutter/rust/aval_graph`, license is preserved.
5. **Git submodules are a pain.** They complicate CI, confuse new contributors, and don't work cleanly with cargo workspaces. Avoid.
6. **Crates.io publication is not our call.** We don't own the aval_graph crate name; the AVAL team does. Submitting upstream to publish is a coordination cost we don't need.

## What we preserve

- **License.** `crates/graph/Cargo.toml` lists `license = "MIT OR Apache-2.0"` (matches upstream).
- **Attribution.** `Cargo.toml` description: "Ring arc + multi-hop route planning, vendored from aval/flutter/rust/aval_graph (AVAL project)."
- **File header comment.** Preserved verbatim.
- **Algorithm.** Byte-equivalent to upstream unless we have an explicit reason to diverge (documented in this file).

## What we don't preserve

- **Crate name.** Upstream is `aval_graph`. Ours is also `aval_graph` (kept identical so attribution is obvious), but if there's a naming conflict later we can rename.
- **Test layout.** Upstream may have tests in a separate `tests/` dir; we keep tests inline as `#[cfg(test)] mod tests` to match this repo's convention.

## Consequences

- **Positive:** Self-contained repo. No external clones required. Stable build (we control when we upgrade).
- **Positive:** Bug fixes in upstream aval_graph can be cherry-picked at our leisure.
- **Negative:** Bug fixes in upstream aval_graph won't automatically reach us. We have to watch the upstream repo.
- **Negative:** We can't contribute bug fixes back via a simple push; we'd submit PRs upstream.
- **Neutral:** License file in `crates/graph/` cites the dual MIT/Apache-2.0 heritage.

## When to revisit

- If upstream aval_graph becomes a crates.io package with a stable release cadence, switch to a versioned dep.
- If we need a feature upstream won't accept (e.g., a different hop-list algorithm), fork into `crates/graph_extended/` and document divergence in an ADR.

## Alternatives considered

**Path-depend** (Option 1): Rejected because it forces cloning the AVAL monorepo.

**Crates.io publication**: Not our call. The AVAL team owns the crate name.

**Git submodule**: Rejected for workspace complexity and CI friction.

## References

- `docs/INTEGRATION-AVAL.md` — full integration plan
- `aval/flutter/rust/aval_graph/src/lib.rs` (upstream source)
- `aval/flutter/rust/aval_graph/Cargo.toml` (upstream metadata)
