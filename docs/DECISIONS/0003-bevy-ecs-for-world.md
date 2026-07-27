# ADR 0003 — bevy_ecs for the world, not the umbrella bevy crate

**Status:** Accepted
**Date:** 2026-07-27
**Decision maker:** John D. Pope

## Context

The world (entities, components, systems) needs an ECS. Options:

1. **`bevy_ecs`** (just the ECS subcrate)
2. **Full `bevy` umbrella crate**
3. **`hecs`**
4. **`specs`**
5. **Hand-rolled ECS**

UE's `UWorld → ULevel → AActor → UActorComponent` hierarchy maps naturally to ECS: World → tagged entity sets → Entity → Component.

## Decision

**Use `bevy_ecs` only.** Do not depend on the `bevy` umbrella crate.

## Rationale

1. **`bevy_ecs` is a mature, fast, well-tested ECS.** It's the same engine Bevy uses internally, extracted as a standalone crate. We get all the ECS performance without the rest of Bevy.
2. **The umbrella crate brings too much.** `bevy = "..."` pulls in `bevy_render`, `bevy_audio`, `bevy_asset`, `bevy_input`, `bevy_ui`, `bevy_text`, `bevy_window`, `bevy_winit`, `bevy_app`, `bevy_core`, and dozens of transitive deps. Most of these conflict directly with our own crates (`crates/render2d/`, `crates/audio/`, `crates/assets/`, `crates/input/`, `crates/ui/`).
3. **Conflict surface with our own subsystems.** If we used `bevy_render`, we'd have two renderers. If we used `bevy_audio`, we'd have two audio engines. Same for assets, input, UI. Each is a war.
4. **`bevy_app`'s plugin system is opinionated.** Bevy apps are configured via `App::new().add_plugins(...)`. Our engine has a different lifecycle (`engine::App::new()` with explicit phase ordering). Mixing the two creates confusion.
5. **`bevy_ecs` is small.** Pulls in `bevy_tasks`, `bevy_utils`, `bevy_reflect` (optional). All small, all useful. `bevy_tasks` gives us a task scheduler for parallel systems; `bevy_utils` is a short stable of helpers. `bevy_reflect` we can disable if we don't want reflection.
6. **`hecs` is simpler but lacks features we want.** `hecs` has no system scheduler, no events, no queries-as-values. For a real engine with N systems per frame, `bevy_ecs`'s `SystemSet` and parallel scheduling are worth the complexity.
7. **`specs` is older and slower-moving.** `bevy_ecs` is the current hotness in Rust ECS-land; community momentum matters for hiring and documentation.
8. **Hand-rolled ECS is reinventing a wheel.** ECS internals are subtle (archetype graphs, sparse-set storage, query batching). Not worth our time.

## Consequences

- **Positive:** We get a fast ECS for ~1 MB of compiled code. `bevy_ecs`'s query DSL (`Query<&mut Transform, With<Walker>>`) is ergonomic. Parallel system scheduling via `bevy_tasks` is free.
- **Negative:** We don't get Bevy's plugin ecosystem (no `bevy_inspector_egui`, no `bevy_tweening`). For tooling we write our own (`tools/inspector`).
- **Neutral:** Some Bevy patterns leak through (e.g., `Commands` for deferred system execution). That's fine; we adopt the patterns we like.

## Cargo.toml shape

```toml
# crates/engine/Cargo.toml
[dependencies]
bevy_ecs = "0.14"
bevy_tasks = "0.14"
# NOT: bevy = "..."
```

We do NOT use `bevy_reflect` by default — reflection is opt-in via a feature flag if a subsystem needs it.

## Alternatives considered

**Full `bevy`**: Rejected because it brings competing subsystems for everything we're building. Each conflict would be resolved by either not using our own crate (waste) or not using Bevy's (then why depend on Bevy?).

**`hecs`**: Rejected for lack of system scheduling and parallel query support.

**`specs`**: Rejected for slower pace and fewer ecosystem tailwinds.

**Hand-rolled**: Rejected as too much engineering for no benefit.

## References

- `docs/DESIGN.md` — Core/World section
- `docs/ARCHITECTURE.md` — Layer diagram (engine crate depends on bevy_ecs)
