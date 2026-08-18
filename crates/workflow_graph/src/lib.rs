//! Static interpretation of Grok Build **Rhai workflows** (`.rhai`).
//!
//! These are not room walk-graphs. They are multi-agent orchestration scripts:
//! `let meta = #{…}`, `phase("…")`, `agent(…)`, `parallel([…])`, `complete(…)`.
//!
//! This crate **does not execute** Rhai. It extracts a structural DAG for
//! visualization / docs (Mermaid, layered layout for wgpu UI).
//!
//! # Pipeline
//!
//! ```text
//! .rhai source
//!    → parse_workflow()     // meta + sequential phases + host calls
//!    → WorkflowGraph
//!    → to_mermaid() | layout_layers() | to_json()
//! ```

#![deny(missing_docs)]

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from static workflow parsing.
#[derive(Debug, Error)]
pub enum WorkflowGraphError {
    /// I/O.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// Source missing required `let meta = #{…}`.
    #[error("missing or invalid let meta = #{{…}} block")]
    MissingMeta,
    /// meta.name missing.
    #[error("meta.name missing")]
    MissingName,
    /// JSON manifest failed to parse (used by [`parse_workflow_manifest`]).
    #[error("manifest json: {0}")]
    ManifestJson(#[from] serde_json::Error),
    /// Manifest was well-formed JSON but missing a required field.
    #[error("manifest field missing: {0}")]
    ManifestField(&'static str),
}

/// Which orchestrator authored / runs the workflow.
///
/// Grok workflows are `.rhai` scripts under `.grok/workflows/`; Claude workflows
/// are JSON manifests under `.claude/workflows/` produced by (or shipped with)
/// the Claude Agent SDK / slash-command ports. Both flatten to the same
/// [`WorkflowGraph`] so the UI renders them identically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    /// `.rhai` workflow parsed from source (default — preserves prior behavior).
    #[default]
    Grok,
    /// `.claude/workflows/*.json` manifest.
    Claude,
}

/// Kind of graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// Workflow entry (meta).
    Start,
    /// `phase("Title")` barrier / rail marker.
    Phase,
    /// Single `agent(...)` call.
    Agent,
    /// `parallel([...])` fan-out barrier.
    Parallel,
    /// `complete(...)` terminal.
    Complete,
    /// `pause(...)` / `await_user(...)` gate.
    Gate,
    /// Unclassified host call we still want on the rail.
    Other,
}

/// One node in the structural graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Stable id (`start`, `phase_0`, `agent_3`, …).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node kind.
    pub kind: NodeKind,
    /// Owning phase title (if any).
    pub phase: Option<String>,
    /// Source line (1-based), when known.
    pub line: Option<usize>,
    /// Extra detail (capability_mode, schema name, job count hint).
    pub detail: Option<String>,
}

/// Directed edge (control-flow order in the authored script).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    /// From node id.
    pub from: String,
    /// To node id.
    pub to: String,
    /// Edge label (optional).
    pub label: Option<String>,
}

/// Phase declared in `meta.phases` and/or first seen via `phase("…")`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaseInfo {
    /// Title (must match `phase("Title")` for the UI rail).
    pub title: String,
    /// Optional detail from meta.
    pub detail: Option<String>,
}

/// Full static model of a workflow script.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowGraph {
    /// `meta.name`.
    pub name: String,
    /// `meta.description`.
    pub description: String,
    /// Optional `meta.when_to_use`.
    pub when_to_use: Option<String>,
    /// Which orchestrator this workflow belongs to.
    #[serde(default)]
    pub backend: Backend,
    /// Declared phases (meta order, then any extra phase() calls).
    pub phases: Vec<PhaseInfo>,
    /// Nodes in roughly source order.
    pub nodes: Vec<GraphNode>,
    /// Sequential control-flow edges.
    pub edges: Vec<GraphEdge>,
    /// Source path if loaded from disk.
    pub source_path: Option<String>,
    /// Rough counts for dashboards.
    pub stats: WorkflowStats,
}

/// Aggregate counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct WorkflowStats {
    /// `phase(` calls found.
    pub phase_calls: usize,
    /// `agent(` calls found.
    pub agent_calls: usize,
    /// `parallel(` calls found.
    pub parallel_calls: usize,
    /// `complete(` calls found.
    pub complete_calls: usize,
    /// `pause(` / `await_user(` calls.
    pub gate_calls: usize,
}

/// 2D layout for game-engine / wgpu drawing (pixel coords, y-down).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutNode {
    /// Node id.
    pub id: String,
    /// Top-left x.
    pub x: f32,
    /// Top-left y.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
    /// Kind (for color).
    pub kind: NodeKind,
    /// Label to draw.
    pub label: String,
}

/// Full layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphLayout {
    /// Positioned nodes.
    pub nodes: Vec<LayoutNode>,
    /// Edges (same as graph).
    pub edges: Vec<GraphEdge>,
    /// Canvas size.
    pub width: f32,
    /// Canvas height.
    pub height: f32,
}

// ── Parse ────────────────────────────────────────────────────────────────

/// Load and parse a `.rhai` workflow file.
pub fn parse_workflow_file(path: impl AsRef<Path>) -> Result<WorkflowGraph, WorkflowGraphError> {
    let path = path.as_ref();
    let src = std::fs::read_to_string(path)?;
    let mut g = parse_workflow(&src)?;
    g.source_path = Some(path.display().to_string());
    Ok(g)
}

/// Parse workflow source text into a structural graph.
pub fn parse_workflow(src: &str) -> Result<WorkflowGraph, WorkflowGraphError> {
    let (name, description, when_to_use, mut phases) = parse_meta(src)?;

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut stats = WorkflowStats::default();

    nodes.push(GraphNode {
        id: "start".into(),
        label: name.clone(),
        kind: NodeKind::Start,
        phase: None,
        line: None,
        detail: Some(truncate(&description, 80)),
    });
    let mut prev = "start".to_string();
    let mut current_phase: Option<String> = None;
    let mut phase_idx = 0usize;
    let mut agent_idx = 0usize;
    let mut parallel_idx = 0usize;
    let mut other_idx = 0usize;

    // Ensure meta phases exist as PhaseInfo
    for p in &phases {
        // phases already filled from meta
        let _ = p;
    }

    for (line_no, line) in src.lines().enumerate() {
        let line_no = line_no + 1;
        let t = line.trim();
        if t.starts_with("//") || t.is_empty() {
            continue;
        }

        // phase("Title") or phase('Title')
        if let Some(title) = extract_phase_title(t) {
            stats.phase_calls += 1;
            current_phase = Some(title.clone());
            if !phases.iter().any(|p| p.title == title) {
                phases.push(PhaseInfo {
                    title: title.clone(),
                    detail: None,
                });
            }
            let id = format!("phase_{phase_idx}");
            phase_idx += 1;
            nodes.push(GraphNode {
                id: id.clone(),
                label: title.clone(),
                kind: NodeKind::Phase,
                phase: Some(title),
                line: Some(line_no),
                detail: None,
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: id.clone(),
                label: None,
            });
            prev = id;
            continue;
        }

        // parallel(  — fan-out node
        if contains_call(t, "parallel") {
            stats.parallel_calls += 1;
            let label = extract_nearby_label(src, line_no).unwrap_or_else(|| {
                format!("parallel×{}", estimate_parallel_jobs(src, line_no).unwrap_or(0))
            });
            let jobs = estimate_parallel_jobs(src, line_no);
            let id = format!("parallel_{parallel_idx}");
            parallel_idx += 1;
            nodes.push(GraphNode {
                id: id.clone(),
                label,
                kind: NodeKind::Parallel,
                phase: current_phase.clone(),
                line: Some(line_no),
                detail: jobs.map(|n| format!("~{n} jobs (heuristic)")),
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: id.clone(),
                label: Some("barrier".into()),
            });
            prev = id;
            continue;
        }

        // agent( — but not inside comments already handled
        if contains_call(t, "agent") && !t.contains("parallel") {
            stats.agent_calls += 1;
            let label = extract_label_from_line(t)
                .or_else(|| extract_nearby_label(src, line_no))
                .unwrap_or_else(|| format!("agent_{agent_idx}"));
            let cap = extract_capability(t);
            let id = format!("agent_{agent_idx}");
            agent_idx += 1;
            nodes.push(GraphNode {
                id: id.clone(),
                label,
                kind: NodeKind::Agent,
                phase: current_phase.clone(),
                line: Some(line_no),
                detail: cap,
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: id.clone(),
                label: None,
            });
            prev = id;
            continue;
        }

        if contains_call(t, "complete") {
            stats.complete_calls += 1;
            let id = format!("complete_{}", stats.complete_calls - 1);
            nodes.push(GraphNode {
                id: id.clone(),
                label: "complete".into(),
                kind: NodeKind::Complete,
                phase: current_phase.clone(),
                line: Some(line_no),
                detail: None,
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: id.clone(),
                label: None,
            });
            prev = id;
            continue;
        }

        if contains_call(t, "pause") || contains_call(t, "await_user") {
            stats.gate_calls += 1;
            let kind_name = if t.contains("await_user") {
                "await_user"
            } else {
                "pause"
            };
            let id = format!("gate_{}", stats.gate_calls - 1);
            nodes.push(GraphNode {
                id: id.clone(),
                label: kind_name.into(),
                kind: NodeKind::Gate,
                phase: current_phase.clone(),
                line: Some(line_no),
                detail: extract_string_arg(t),
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: id.clone(),
                label: Some("pause".into()),
            });
            prev = id;
            continue;
        }

        // log / write_scratch — light “other” nodes only if clearly host calls
        if contains_call(t, "write_scratch_file") {
            let id = format!("other_{other_idx}");
            other_idx += 1;
            nodes.push(GraphNode {
                id: id.clone(),
                label: "write_scratch".into(),
                kind: NodeKind::Other,
                phase: current_phase.clone(),
                line: Some(line_no),
                detail: extract_string_arg(t),
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: id.clone(),
                label: None,
            });
            prev = id;
        }
    }

    Ok(WorkflowGraph {
        name,
        description,
        when_to_use,
        backend: Backend::Grok,
        phases,
        nodes,
        edges,
        source_path: None,
        stats,
    })
}

/// Load and parse a `.claude/workflows/*.json` manifest.
///
/// See [`parse_workflow_manifest`] for the schema and how each manifest phase
/// unfolds into the shared [`WorkflowGraph`].
pub fn parse_workflow_manifest_file(
    path: impl AsRef<Path>,
) -> Result<WorkflowGraph, WorkflowGraphError> {
    let path = path.as_ref();
    let src = std::fs::read_to_string(path)?;
    let mut g = parse_workflow_manifest(&src)?;
    g.source_path = Some(path.display().to_string());
    Ok(g)
}

/// Parse a Claude workflow JSON manifest into a [`WorkflowGraph`].
///
/// Expected shape (v1 — see `scroll-world/.claude/workflows/*.json`):
///
/// ```json
/// {
///   "backend": "claude",
///   "name": "scroll-world",
///   "description": "…",
///   "when_to_use": "…",
///   "phases": [
///     {
///       "title": "Bootstrap",
///       "detail": "CLI tools + balances",
///       "agents": [{"label": "bootstrap", "capability_mode": "execute"}],
///       "gates":  [{"kind": "infra", "when": "on_failure", "message": "…"}]
///     }
///   ]
/// }
/// ```
///
/// The parser walks each phase in declared order emitting a phase node, then
/// one `Agent` node per `agents[]` entry, then one `Gate` node per `gates[]`
/// entry. Every phase also seeds a [`PhaseInfo`] so the rail matches the DAG.
pub fn parse_workflow_manifest(src: &str) -> Result<WorkflowGraph, WorkflowGraphError> {
    let v: serde_json::Value = serde_json::from_str(src)?;
    let obj = v.as_object().ok_or(WorkflowGraphError::ManifestField("root"))?;

    let name = obj
        .get("name")
        .and_then(|x| x.as_str())
        .ok_or(WorkflowGraphError::ManifestField("name"))?
        .to_string();
    let description = obj
        .get("description")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let when_to_use = obj
        .get("when_to_use")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());

    let mut phases: Vec<PhaseInfo> = Vec::new();
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut stats = WorkflowStats::default();

    nodes.push(GraphNode {
        id: "start".into(),
        label: name.clone(),
        kind: NodeKind::Start,
        phase: None,
        line: None,
        detail: Some(truncate(&description, 80)),
    });
    let mut prev = "start".to_string();
    let mut phase_idx = 0usize;
    let mut agent_idx = 0usize;
    let mut gate_idx = 0usize;

    if let Some(pl) = obj.get("phases").and_then(|x| x.as_array()) {
        for p in pl {
            let title = p
                .get("title")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if title.is_empty() {
                continue;
            }
            let detail = p
                .get("detail")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            phases.push(PhaseInfo {
                title: title.clone(),
                detail: detail.clone(),
            });
            stats.phase_calls += 1;
            let pid = format!("phase_{phase_idx}");
            phase_idx += 1;
            nodes.push(GraphNode {
                id: pid.clone(),
                label: title.clone(),
                kind: NodeKind::Phase,
                phase: Some(title.clone()),
                line: None,
                detail,
            });
            edges.push(GraphEdge {
                from: prev.clone(),
                to: pid.clone(),
                label: None,
            });
            prev = pid;

            if let Some(agents) = p.get("agents").and_then(|x| x.as_array()) {
                for a in agents {
                    stats.agent_calls += 1;
                    let label = a
                        .get("label")
                        .and_then(|x| x.as_str())
                        .unwrap_or("agent")
                        .to_string();
                    let cap = a
                        .get("capability_mode")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    let id = format!("agent_{agent_idx}");
                    agent_idx += 1;
                    nodes.push(GraphNode {
                        id: id.clone(),
                        label,
                        kind: NodeKind::Agent,
                        phase: Some(title.clone()),
                        line: None,
                        detail: cap,
                    });
                    edges.push(GraphEdge {
                        from: prev.clone(),
                        to: id.clone(),
                        label: None,
                    });
                    prev = id;
                }
            }

            if let Some(gates) = p.get("gates").and_then(|x| x.as_array()) {
                for g in gates {
                    stats.gate_calls += 1;
                    let kind_name = g
                        .get("kind")
                        .and_then(|x| x.as_str())
                        .unwrap_or("gate")
                        .to_string();
                    let msg = g
                        .get("message")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string());
                    let id = format!("gate_{gate_idx}");
                    gate_idx += 1;
                    nodes.push(GraphNode {
                        id: id.clone(),
                        label: kind_name.clone(),
                        kind: NodeKind::Gate,
                        phase: Some(title.clone()),
                        line: None,
                        detail: msg,
                    });
                    // A gate that only fires on failure is still on the rail
                    // — the UI can dim it if the run bypassed it. `when` is
                    // reserved for that logic; not part of the DAG shape.
                    let _ = g.get("when");
                    edges.push(GraphEdge {
                        from: prev.clone(),
                        to: id.clone(),
                        label: Some("gate".into()),
                    });
                    prev = id;
                }
            }
        }
    }

    Ok(WorkflowGraph {
        name,
        description,
        when_to_use,
        backend: Backend::Claude,
        phases,
        nodes,
        edges,
        source_path: None,
        stats,
    })
}

fn parse_meta(
    src: &str,
) -> Result<(String, String, Option<String>, Vec<PhaseInfo>), WorkflowGraphError> {
    // Find `let meta = #{`
    let start = src
        .find("let meta")
        .ok_or(WorkflowGraphError::MissingMeta)?;
    let brace = src[start..]
        .find("#{")
        .ok_or(WorkflowGraphError::MissingMeta)?
        + start;
    let block = extract_balanced(&src[brace..], '#', '{', '}')
        .ok_or(WorkflowGraphError::MissingMeta)?;
    // block includes #{ … }
    let name = extract_map_string(block, "name").ok_or(WorkflowGraphError::MissingName)?;
    let description = extract_map_string(block, "description").unwrap_or_default();
    let when_to_use = extract_map_string(block, "when_to_use");
    let phases = extract_meta_phases(block);
    Ok((name, description, when_to_use, phases))
}

fn extract_balanced<'a>(
    s: &'a str,
    _lead: char,
    open: char,
    close: char,
) -> Option<&'a str> {
    // s starts at #{
    let bytes = s.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'#' || bytes[1] != open as u8 {
        // try starting at {
        let start = s.find(open)?;
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escape = false;
        for (i, ch) in s[start..].char_indices() {
            if in_str {
                if escape {
                    escape = false;
                } else if ch == '\\' {
                    escape = true;
                } else if ch == '"' {
                    in_str = false;
                }
                continue;
            }
            match ch {
                '"' => in_str = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&s[start..=start + i]);
                    }
                }
                _ => {}
            }
        }
        return None;
    }
    // #{
    let start = 1; // at {
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, ch) in s[start..].char_indices() {
        if in_str {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    // include leading #
                    return Some(&s[0..=start + i]);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_map_string(block: &str, key: &str) -> Option<String> {
    // name: "foo" or name: "multi…
    let pat = format!("{key}:");
    let idx = block.find(&pat)?;
    let rest = block[idx + pat.len()..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = rest[1..].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else if c == '"' {
            break;
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn extract_meta_phases(block: &str) -> Vec<PhaseInfo> {
    let mut out = Vec::new();
    // Look for title: "…" inside phases: [ … ]
    let Some(pstart) = block.find("phases:") else {
        return out;
    };
    let rest = &block[pstart..];
    let Some(bracket) = rest.find('[') else {
        return out;
    };
    let slice = &rest[bracket..];
    // naive scan for title: "..."
    let mut search = slice;
    while let Some(t) = search.find("title:") {
        let after = search[t + 6..].trim_start();
        if let Some(title) = parse_quoted(after) {
            // optional detail nearby
            let window = &search[t..t.saturating_add(200).min(search.len())];
            let detail = window
                .find("detail:")
                .and_then(|d| parse_quoted(window[d + 7..].trim_start()));
            out.push(PhaseInfo { title, detail });
        }
        search = &search[t + 6..];
    }
    out
}

fn parse_quoted(s: &str) -> Option<String> {
    if !s.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = s[1..].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(n) = chars.next() {
                out.push(n);
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

fn extract_phase_title(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with("phase(") {
        return None;
    }
    parse_quoted(t["phase(".len()..].trim_start())
        .or_else(|| {
            // phase("Explore");
            let inner = t.strip_prefix("phase(")?.trim_end_matches(';').trim_end_matches(')');
            parse_quoted(inner.trim())
        })
}

fn contains_call(line: &str, name: &str) -> bool {
    // word-boundary-ish: name(
    let pat = format!("{name}(");
    if let Some(i) = line.find(&pat) {
        // not part of a longer identifier
        if i > 0 {
            let prev = line.as_bytes()[i - 1] as char;
            if prev.is_ascii_alphanumeric() || prev == '_' {
                return false;
            }
        }
        return true;
    }
    false
}

fn extract_label_from_line(line: &str) -> Option<String> {
    // label: "foo"
    let idx = line.find("label:")?;
    parse_quoted(line[idx + 6..].trim_start())
}

fn extract_capability(line: &str) -> Option<String> {
    let idx = line.find("capability_mode:")?;
    parse_quoted(line[idx + 16..].trim_start())
}

fn extract_string_arg(line: &str) -> Option<String> {
    // first quoted string on the line
    let idx = line.find('"')?;
    parse_quoted(&line[idx..])
}

fn extract_nearby_label(src: &str, line_no: usize) -> Option<String> {
    let lines: Vec<&str> = src.lines().collect();
    // Search current line and next 8 lines for label:
    let start = line_no.saturating_sub(1);
    let end = (start + 9).min(lines.len());
    for l in &lines[start..end] {
        if let Some(lab) = extract_label_from_line(l) {
            return Some(lab);
        }
    }
    None
}

/// Heuristic: count job maps between this `parallel(` line and its close.
///
/// Prefers `label:` hits; falls back to `.push(#{` / `jobs.push` style builders.
fn estimate_parallel_jobs(src: &str, line_no: usize) -> Option<usize> {
    let lines: Vec<&str> = src.lines().collect();
    let start = line_no.saturating_sub(1);
    let mut depth = 0i32;
    let mut labels = 0usize;
    let mut pushes = 0usize;
    let mut seen_open = false;
    for l in lines.iter().skip(start).take(120) {
        for ch in l.chars() {
            if ch == '(' || ch == '[' {
                depth += 1;
                seen_open = true;
            } else if ch == ')' || ch == ']' {
                depth -= 1;
            }
        }
        if l.contains("label:") {
            labels += 1;
        }
        if l.contains(".push(") || l.contains("push(#{") {
            pushes += 1;
        }
        if seen_open && depth <= 0 {
            break;
        }
    }
    // Also scan *above* parallel() for job-building loops (common pattern:
    // build jobs, then `parallel(jobs)`).
    if labels == 0 && pushes == 0 {
        let back = start.saturating_sub(40);
        for l in &lines[back..start] {
            if l.contains("label:") {
                labels += 1;
            }
            if l.contains(".push(") || l.contains("push(#{") {
                pushes += 1;
            }
        }
    }
    let n = labels.max(pushes);
    if n > 0 {
        Some(n)
    } else {
        None
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

// ── Export ───────────────────────────────────────────────────────────────

impl WorkflowGraph {
    /// Mermaid `flowchart TD` for docs / GitHub.
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("```mermaid\nflowchart TD\n");
        out.push_str(&format!(
            "  classDef phase fill:#2a3340,stroke:#c4a574,color:#e7e9ea\n"
        ));
        out.push_str(&format!(
            "  classDef agent fill:#1a3a2a,stroke:#3d9a6a,color:#e7e9ea\n"
        ));
        out.push_str(&format!(
            "  classDef parallel fill:#1a2a3a,stroke:#5b9fd4,color:#e7e9ea\n"
        ));
        out.push_str(&format!(
            "  classDef gate fill:#3a2a1a,stroke:#c45c5c,color:#e7e9ea\n"
        ));
        out.push_str(&format!(
            "  classDef start fill:#141a22,stroke:#8b98a5,color:#e7e9ea\n"
        ));

        for n in &self.nodes {
            let shape_open = match n.kind {
                NodeKind::Start | NodeKind::Complete => "([",
                NodeKind::Phase => "[",
                NodeKind::Parallel => "{{",
                NodeKind::Gate => "[/",
                NodeKind::Agent | NodeKind::Other => "(",
            };
            let shape_close = match n.kind {
                NodeKind::Start | NodeKind::Complete => "])",
                NodeKind::Phase => "]",
                NodeKind::Parallel => "}}",
                NodeKind::Gate => "/]",
                NodeKind::Agent | NodeKind::Other => ")",
            };
            let label = mermaid_escape(&n.label);
            out.push_str(&format!(
                "  {}{}{}{}\n",
                n.id, shape_open, label, shape_close
            ));
            let class = match n.kind {
                NodeKind::Phase => "phase",
                NodeKind::Agent => "agent",
                NodeKind::Parallel => "parallel",
                NodeKind::Gate => "gate",
                NodeKind::Start | NodeKind::Complete | NodeKind::Other => "start",
            };
            out.push_str(&format!("  class {} {}\n", n.id, class));
        }
        for e in &self.edges {
            if let Some(ref lab) = e.label {
                out.push_str(&format!(
                    "  {} -->|{}| {}\n",
                    e.from,
                    mermaid_escape(lab),
                    e.to
                ));
            } else {
                out.push_str(&format!("  {} --> {}\n", e.from, e.to));
            }
        }
        out.push_str("```\n");
        out
    }

    /// Compact JSON for tools / Flutter / CMS.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Layered left-to-right layout (y-down pixel space) for wgpu / UI.
    ///
    /// Columns ≈ phases; agents/parallels stack under their phase.
    pub fn layout_layers(&self, node_w: f32, node_h: f32, gap_x: f32, gap_y: f32) -> GraphLayout {
        let mut laid: Vec<LayoutNode> = Vec::new();
        let margin = 24.0_f32;

        for n in &self.nodes {
            let (c, r) = match n.kind {
                NodeKind::Start => (0, 0),
                NodeKind::Phase => {
                    let pi = self
                        .phases
                        .iter()
                        .position(|p| p.title == n.label)
                        .unwrap_or(0);
                    (pi + 1, 0)
                }
                NodeKind::Complete => (self.phases.len().saturating_add(2), 0),
                _ => {
                    let pi = n
                        .phase
                        .as_ref()
                        .and_then(|ph| self.phases.iter().position(|p| &p.title == ph))
                        .unwrap_or(0);
                    let col = pi + 1;
                    let stack = laid
                        .iter()
                        .filter(|l| {
                            let lcol = match l.kind {
                                NodeKind::Start => 0,
                                NodeKind::Phase | NodeKind::Complete => {
                                    // don't count
                                    return false;
                                }
                                _ => {
                                    // recover column from x
                                    ((l.x - margin) / (node_w + gap_x)).round() as usize
                                }
                            };
                            lcol == col
                        })
                        .count();
                    (col, stack + 1)
                }
            };

            laid.push(LayoutNode {
                id: n.id.clone(),
                x: margin + c as f32 * (node_w + gap_x),
                y: margin + r as f32 * (node_h + gap_y),
                w: node_w,
                h: node_h,
                kind: n.kind,
                label: n.label.clone(),
            });
        }

        let width = laid.iter().map(|n| n.x + n.w).fold(margin, f32::max) + margin;
        let height = laid.iter().map(|n| n.y + n.h).fold(margin, f32::max) + margin;

        GraphLayout {
            nodes: laid,
            edges: self.edges.clone(),
            width,
            height,
        }
    }

    /// Human-readable summary (CLI / status strip).
    pub fn summary_line(&self) -> String {
        format!(
            "{} — {} phases · {} agents · {} parallels · {} completes",
            self.name,
            self.phases.len(),
            self.stats.agent_calls,
            self.stats.parallel_calls,
            self.stats.complete_calls
        )
    }
}

fn mermaid_escape(s: &str) -> String {
    s.replace('"', "'")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', " ")
}

// ── Tint helpers for UI ──────────────────────────────────────────────────

/// Suggested RGBA (0–1) per node kind — matches Ariadne dark chrome.
pub fn kind_color(kind: NodeKind) -> (f32, f32, f32, f32) {
    match kind {
        NodeKind::Start | NodeKind::Complete => (0.15, 0.18, 0.22, 0.95),
        NodeKind::Phase => (0.20, 0.24, 0.30, 0.95),
        NodeKind::Agent => (0.12, 0.28, 0.20, 0.95),
        NodeKind::Parallel => (0.12, 0.20, 0.32, 0.95),
        NodeKind::Gate => (0.32, 0.18, 0.14, 0.95),
        NodeKind::Other => (0.18, 0.18, 0.22, 0.90),
    }
}

/// Border color (gold / accent).
pub fn kind_border(kind: NodeKind) -> (f32, f32, f32, f32) {
    match kind {
        NodeKind::Phase => (0.77, 0.65, 0.45, 1.0),
        NodeKind::Agent => (0.24, 0.60, 0.42, 1.0),
        NodeKind::Parallel => (0.36, 0.62, 0.83, 1.0),
        NodeKind::Gate => (0.77, 0.36, 0.36, 1.0),
        _ => (0.55, 0.60, 0.65, 0.9),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
let meta = #{
    name: "review-changes",
    description: "Review a diff across dimensions",
    when_to_use: "after a PR",
    phases: [
        #{ title: "Review", detail: "one reviewer per dimension" },
        #{ title: "Verify", detail: "one skeptic per finding" },
    ],
};

phase("Review");
let jobs = [];
jobs.push(#{
    prompt: "Review code",
    label: "review:correctness",
    capability_mode: "read-only",
});
let results = parallel(jobs);

phase("Verify");
let r = agent("Verify findings", #{ label: "verify:main", capability_mode: "read-only" });
if r.success { complete(r.output); }
"#;

    #[test]
    fn parses_meta_and_phases() {
        let g = parse_workflow(SAMPLE).expect("parse");
        assert_eq!(g.name, "review-changes");
        assert!(g.description.contains("Review a diff"));
        assert_eq!(g.phases.len(), 2);
        assert_eq!(g.phases[0].title, "Review");
        assert!(g.stats.phase_calls >= 2);
        assert!(g.stats.parallel_calls >= 1);
        assert!(g.stats.agent_calls >= 1);
        assert!(g.stats.complete_calls >= 1);
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Start));
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Parallel));
        assert!(!g.edges.is_empty());
    }

    #[test]
    fn mermaid_contains_flowchart() {
        let g = parse_workflow(SAMPLE).unwrap();
        let m = g.to_mermaid();
        assert!(m.contains("flowchart TD"));
        assert!(m.contains("review-changes") || m.contains("start"));
    }

    #[test]
    fn layout_has_positive_size() {
        let g = parse_workflow(SAMPLE).unwrap();
        let lay = g.layout_layers(140.0, 48.0, 32.0, 16.0);
        assert!(lay.width > 100.0);
        assert!(lay.height > 40.0);
        assert_eq!(lay.nodes.len(), g.nodes.len());
    }

    const CLAUDE_MANIFEST: &str = r#"
    {
      "backend": "claude",
      "name": "scroll-world",
      "description": "test build",
      "when_to_use": "when",
      "phases": [
        {"title": "Bootstrap", "detail": "cli",
         "agents": [{"label": "bootstrap", "capability_mode": "execute"}],
         "gates":  [{"kind": "infra", "when": "on_failure", "message": "install cli"}]},
        {"title": "Approve",
         "gates":  [{"kind": "user", "when": "always_unless_auto_approve", "message": "approve?"}]}
      ]
    }
    "#;

    #[test]
    fn parses_claude_manifest() {
        let g = parse_workflow_manifest(CLAUDE_MANIFEST).expect("parse manifest");
        assert_eq!(g.backend, Backend::Claude);
        assert_eq!(g.name, "scroll-world");
        assert_eq!(g.phases.len(), 2);
        assert!(g.stats.agent_calls >= 1);
        assert!(g.stats.gate_calls >= 2);
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Phase));
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Agent));
        assert!(g.nodes.iter().any(|n| n.kind == NodeKind::Gate));
    }

    #[test]
    fn backend_default_is_grok() {
        let g = parse_workflow(SAMPLE).unwrap();
        assert_eq!(g.backend, Backend::Grok);
    }

    #[test]
    fn parse_real_game_engine_demos_if_present() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"
        );
        // Also try relative from adventure-engine sibling
        let alt = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"
        );
        let p = if std::path::Path::new(path).is_file() {
            path
        } else if std::path::Path::new(alt).is_file() {
            alt
        } else {
            // Documents path
            "/home/johndpope/Documents/GitHub/PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"
        };
        if !std::path::Path::new(p).is_file() {
            return;
        }
        let g = parse_workflow_file(p).expect("parse game-engine-demos");
        assert_eq!(g.name, "game-engine-demos");
        assert!(g.phases.len() >= 4);
        assert!(g.stats.parallel_calls >= 1);
        println!("{}", g.summary_line());
        println!("{}", g.to_mermaid());
    }
}
