# Example 09 — Rhai workflow graph (Rust backend)

**Interpret** Grok Build `.rhai` workflows (static structure). UI is Flutter —
see sibling repo `workflow_graph_ui/` (talks to this process over HTTP).

| Mode | Output |
| --- | --- |
| default | Mermaid markdown |
| `--json` | `WorkflowGraph` JSON |
| `--serve` | HTTP API for Flutter (default `:8791`) |

Not a Rhai runtime — extracts `meta` / `phase` / `agent` / `parallel` / gates.

## Run

```bash
# Mermaid / JSON
cargo run -p example-09-workflow-graph -- path/to/w.rhai
cargo run -p example-09-workflow-graph -- --json path/to/w.rhai

# Backend for Flutter UI
cargo run -p example-09-workflow-graph -- --serve --port 8791

# Then in PresidentialDilema-FastApi:
#   cd workflow_graph_ui && flutter run -d chrome
```

### HTTP

| Method | Path |
| --- | --- |
| GET | `/health` |
| GET | `/v1/workflows?dir=…` |
| GET | `/v1/parse?path=…` |
| POST | `/v1/parse` (body = source) |
| GET | `/v1/layout?path=…` |
| GET | `/v1/mermaid?path=…` |
| GET | `/v1/runs` | Grok + Claude + shell pipeline (`STATUS.json`) |
| GET | `/v1/logs?source=rsrt_overnight&n=200` | Allowlisted log tail |

CORS `*` for local Flutter web.

## Library

```rust
use adventure_workflow_graph::parse_workflow_file;
let g = parse_workflow_file("workflow.rhai")?;
println!("{}", g.to_json_pretty()?);
```

Crate: `crates/workflow_graph`. Snapshots: [`docs/workflows/`](../../docs/workflows/).
