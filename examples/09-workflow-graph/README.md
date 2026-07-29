# Example 09 — Rhai workflow graph

**Interpret** Grok Build `.rhai` workflows (static structure) and **visualize** them:

| Mode | Output |
| --- | --- |
| default | Mermaid markdown (GitHub-ready) |
| `--json` | Structural JSON (`WorkflowGraph`) |
| `--window` | wgpu DAG viewer (Ariadne render2d) |

This is **not** a Rhai runtime. It extracts `meta`, `phase()`, `agent()`, `parallel()`, `complete()`, `pause`/`await_user` into a control-flow graph.

## Run

```bash
# From adventure-engine root — default sample if sibling FastApi repo exists
cargo run -p example-09-workflow-graph -- \
  ../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai

# JSON
cargo run -p example-09-workflow-graph -- --json path/to/w.rhai

# Live DAG (drag to pan, +/- zoom, M logs mermaid, Esc quit)
cargo run -p example-09-workflow-graph -- --window path/to/w.rhai

# Write docs
cargo run -p example-09-workflow-graph -- \
  --out docs/workflows/game-engine-demos.md \
  ../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai
```

## Library

```rust
use adventure_workflow_graph::parse_workflow_file;

let g = parse_workflow_file("workflow.rhai")?;
println!("{}", g.summary_line());
println!("{}", g.to_mermaid());
let layout = g.layout_layers(160.0, 52.0, 40.0, 20.0);
```

Crate: `crates/workflow_graph` (`adventure-workflow-graph`).

## Project workflows (checked-in Mermaid)

Generated snapshots under [`docs/workflows/`](../../docs/workflows/):

- [game-engine-demos](../../docs/workflows/game-engine-demos.md)
- [shawshank-ncp-game](../../docs/workflows/shawshank-ncp-game.md)
- [aval-cms-assets](../../docs/workflows/aval-cms-assets.md)
- [site-upgrade](../../docs/workflows/site-upgrade.md)

Regenerate after editing `.rhai` files in `PresidentialDilema-FastApi/.grok/workflows/`.
