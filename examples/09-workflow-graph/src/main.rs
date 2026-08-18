//! Example 09 — **Rhai workflow interpreter** (CLI + local HTTP for Flutter)
//!
//! Static-parse Grok Build `.rhai` workflows into a control-flow DAG.
//! Flutter (or any UI) talks to this process — Rust stays under the hood.
//!
//! ```text
//! # Mermaid / JSON
//! cargo run -p example-09-workflow-graph -- path/to/w.rhai
//! cargo run -p example-09-workflow-graph -- --json path/to/w.rhai
//!
//! # HTTP API for Flutter (default :8791)
//! cargo run -p example-09-workflow-graph -- --serve
//! cargo run -p example-09-workflow-graph -- --serve --port 8791
//!
//! GET  /health
//! GET  /v1/workflows?dir=…          list .rhai basenames
//! GET  /v1/parse?path=…             parse file → WorkflowGraph JSON
//! POST /v1/parse  body: rhai text   parse source → JSON
//! GET  /v1/mermaid?path=…
//! GET  /v1/runs                     live Grok/Claude + pipeline STATUS.json
//! GET  /v1/logs?source=…|path=…     allowlisted log tail (rsrt overnight, journals)
//! GET  /v1/layout?path=…
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

use adventure_workflow_graph::{
    parse_workflow, parse_workflow_file, parse_workflow_manifest, parse_workflow_manifest_file,
    WorkflowGraph,
};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut json = false;
    let mut serve = false;
    let mut port: u16 = 8791;
    let mut out_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" | "-j" => {
                json = true;
                args.remove(i);
            }
            "--serve" | "-s" => {
                serve = true;
                args.remove(i);
            }
            "--port" | "-p" => {
                args.remove(i);
                if i < args.len() {
                    port = args[i].parse().unwrap_or(8791);
                    args.remove(i);
                }
            }
            "--out" | "-o" => {
                args.remove(i);
                if i < args.len() {
                    out_path = Some(PathBuf::from(&args[i]));
                    args.remove(i);
                }
            }
            "-h" | "--help" => {
                print_help();
                return;
            }
            _ => i += 1,
        }
    }

    if serve {
        run_server(port);
        return;
    }

    let path = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(default_workflow_path);

    if !path.is_file() {
        eprintln!("workflow not found: {}", path.display());
        eprintln!("pass a path to a .rhai workflow (see --help)");
        std::process::exit(1);
    }

    let graph = parse_by_extension(&path.to_string_lossy()).unwrap_or_else(|e| {
        eprintln!("parse error: {e}");
        std::process::exit(1);
    });

    eprintln!("{}", graph.summary_line());
    let body = format_output(&graph, json);
    if let Some(out) = out_path {
        std::fs::write(&out, &body).expect("write --out");
        eprintln!("wrote {}", out.display());
    } else {
        print!("{body}");
    }
}

fn format_output(graph: &WorkflowGraph, json: bool) -> String {
    if json {
        graph.to_json_pretty().expect("json")
    } else {
        let mut md = format!("# {}\n\n{}\n\n", graph.name, graph.description);
        if let Some(ref w) = graph.when_to_use {
            md.push_str(&format!("**When:** {w}\n\n"));
        }
        md.push_str("## Stats\n\n");
        md.push_str(&format!(
            "| metric | count |\n| --- | --- |\n| phases | {} |\n| phase() | {} |\n| agent() | {} |\n| parallel() | {} |\n| complete() | {} |\n| gates | {} |\n\n",
            graph.phases.len(),
            graph.stats.phase_calls,
            graph.stats.agent_calls,
            graph.stats.parallel_calls,
            graph.stats.complete_calls,
            graph.stats.gate_calls,
        ));
        md.push_str("## Graph\n\n");
        md.push_str(&graph.to_mermaid());
        md
    }
}

fn print_help() {
    eprintln!(
        "example-09-workflow-graph — static Rhai workflow → Mermaid / JSON / HTTP\n\n\
         USAGE:\n\
           cargo run -p example-09-workflow-graph -- [FLAGS] [workflow.rhai]\n\n\
         FLAGS:\n\
           --json, -j         emit JSON (for Flutter / tools)\n\
           --serve, -s        HTTP API on --port (default 8791)\n\
           --port, -p N       listen port for --serve\n\
           --out, -o PATH     write output to PATH\n\
           -h, --help         this help\n\n\
         HTTP:\n\
           GET  /health\n\
           GET  /v1/workflows?dir=/path/to/.grok/workflows      (rhai only)\n\
           GET  /v1/workflows?repo=/path/to/repo                (rhai + json, tagged backend)\n\
           GET  /v1/parse?path=/abs/file.rhai|json\n\
           POST /v1/parse     body = rhai source OR json manifest (auto-detected)\n\
           GET  /v1/mermaid?path=…\n\
           GET  /v1/layout?path=…&w=180&h=56\n\
           GET  /v1/runs?root=…&claude_log=…   grok + claude + pipeline STATUS (merged)\n\
           GET  /v1/logs?source=rsrt_overnight&n=200&offset=0\n\
           GET  /v1/logs?path=/abs/allowlisted.log&n=200&offset=0\n"
    );
}

fn default_workflow_path() -> PathBuf {
    let candidates = [
        PathBuf::from("../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"),
        PathBuf::from("../../PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai"),
        PathBuf::from(
            "/home/johndpope/Documents/GitHub/PresidentialDilema-FastApi/.grok/workflows/game-engine-demos.rhai",
        ),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("workflow.rhai"))
}

fn default_workflows_dir() -> PathBuf {
    let candidates = [
        PathBuf::from(
            "/home/johndpope/Documents/GitHub/PresidentialDilema-FastApi/.grok/workflows",
        ),
        PathBuf::from("../PresidentialDilema-FastApi/.grok/workflows"),
        PathBuf::from("../../PresidentialDilema-FastApi/.grok/workflows"),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_dir())
        .map(|p| p.canonicalize().unwrap_or(p))
        .unwrap_or_else(|| PathBuf::from("."))
}

// ── Minimal HTTP (no extra deps) ─────────────────────────────────────────

fn run_server(port: u16) {
    // 0.0.0.0 so z6 nginx can reach this box (graph.scrya.com → MSI:8791)
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("bind {addr}: {e}");
        std::process::exit(1);
    });
    eprintln!("workflow-graph API  http://{addr}  (LAN :{port})");
    eprintln!("  GET /health");
    eprintln!("  GET /v1/workflows?dir=…               (rhai)");
    eprintln!("  GET /v1/workflows?repo=…              (rhai + claude json, tagged backend)");
    eprintln!("  GET /v1/parse?path=…                  (auto-dispatch .rhai vs .json)");
    eprintln!("  POST /v1/parse  (body = rhai OR json manifest, auto-detected)");
    eprintln!("  GET /v1/runs?root=…&claude_log=…      (grok + claude + pipeline STATUS)");
    eprintln!("  GET /v1/logs?source=rsrt_overnight     (or ?path= allowlisted tail)");
    eprintln!("CORS: *  (Flutter web local)");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                thread::spawn(move || {
                    if let Err(e) = handle_client(s) {
                        eprintln!("request error: {e}");
                    }
                });
            }
            Err(e) => eprintln!("accept: {e}"),
        }
    }
}

fn handle_client(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 65536];
    let n = stream.read(&mut buf)?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let target = parts.next().unwrap_or("/");

    // headers + body
    let mut content_length = 0usize;
    let mut body_start = 0usize;
    if let Some(idx) = req.find("\r\n\r\n") {
        body_start = idx + 4;
        for h in req[..idx].lines().skip(1) {
            let hl = h.to_ascii_lowercase();
            if let Some(v) = hl.strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = if body_start > 0 && body_start <= n {
        req[body_start..n].to_string()
    } else {
        String::new()
    };
    // read rest if needed
    while body.len() < content_length {
        let mut more = [0u8; 8192];
        let m = stream.read(&mut more)?;
        if m == 0 {
            break;
        }
        body.push_str(&String::from_utf8_lossy(&more[..m]));
    }

    let (path_only, query) = split_query(target);

    let (status, content_type, payload) = match (method, path_only) {
        ("GET", "/health") | ("GET", "/") => (
            "200 OK",
            "application/json",
            r#"{"ok":true,"service":"workflow-graph"}"#.to_string(),
        ),
        ("OPTIONS", _) => ("204 No Content", "text/plain", String::new()),
        ("GET", "/v1/workflows") => {
            let repo_s = query_param(query, "repo");
            let dir_s = query_param(query, "dir");
            let result = if !repo_s.is_empty() {
                list_workflows_repo(&PathBuf::from(repo_s))
            } else if !dir_s.is_empty() {
                list_workflows(&PathBuf::from(dir_s))
            } else {
                list_workflows(&default_workflows_dir())
            };
            match result {
                Ok(j) => ("200 OK", "application/json", j),
                Err(e) => (
                    "400 Bad Request",
                    "application/json",
                    format!(r#"{{"error":{}}}"#, json_str(&e)),
                ),
            }
        }
        ("GET", "/v1/parse") => {
            let path = query_param(query, "path");
            if path.is_empty() {
                (
                    "400 Bad Request",
                    "application/json",
                    r#"{"error":"missing path"}"#.into(),
                )
            } else {
                match parse_by_extension(&path) {
                    Ok(g) => (
                        "200 OK",
                        "application/json",
                        g.to_json_pretty().unwrap_or_else(|_| "{}".into()),
                    ),
                    Err(e) => (
                        "400 Bad Request",
                        "application/json",
                        format!(r#"{{"error":{}}}"#, json_str(&e.to_string())),
                    ),
                }
            }
        }
        ("POST", "/v1/parse") => {
            // Try the manifest parser only if the body looks like JSON; otherwise Rhai.
            // Trim leading whitespace so `\n{…}` still routes to JSON.
            let trimmed = body.trim_start();
            let is_json = trimmed.starts_with('{') || trimmed.starts_with('[');
            let parsed = if is_json {
                parse_workflow_manifest(&body).map_err(|e| e.to_string())
            } else {
                parse_workflow(&body).map_err(|e| e.to_string())
            };
            match parsed {
                Ok(g) => (
                    "200 OK",
                    "application/json",
                    g.to_json_pretty().unwrap_or_else(|_| "{}".into()),
                ),
                Err(e) => (
                    "400 Bad Request",
                    "application/json",
                    format!(r#"{{"error":{}}}"#, json_str(&e)),
                ),
            }
        }
        ("GET", "/v1/runs") => {
            // Live Grok runs (session state.json under ~/.grok/sessions) +
            // Claude runs from a JSONL log (~/.claude-workflows/runs.jsonl) +
            // shell pipelines (e.g. rsrt_overnight STATUS.json).
            let root = query_param(query, "root");
            let root_path = if root.is_empty() {
                default_runs_root()
            } else {
                PathBuf::from(root)
            };
            let claude_log_q = query_param(query, "claude_log");
            let claude_log = if claude_log_q.is_empty() {
                default_claude_log_path()
            } else {
                PathBuf::from(claude_log_q)
            };
            match list_all_runs(&root_path, &claude_log) {
                Ok(j) => ("200 OK", "application/json", j),
                Err(e) => (
                    "400 Bad Request",
                    "application/json",
                    format!(r#"{{"error":{}}}"#, json_str(&e)),
                ),
            }
        }
        ("GET", "/v1/logs") => {
            // Allowlisted log tail for web UI (rsrt overnight, or explicit path).
            let source = query_param(query, "source");
            let path_q = query_param(query, "path");
            let n: usize = query_param(query, "n").parse().unwrap_or(200).clamp(1, 2000);
            let offset: Option<u64> = {
                let o = query_param(query, "offset");
                if o.is_empty() {
                    None
                } else {
                    o.parse().ok()
                }
            };
            match serve_log_tail(&source, &path_q, n, offset) {
                Ok(j) => ("200 OK", "application/json", j),
                Err(e) => (
                    "400 Bad Request",
                    "application/json",
                    format!(r#"{{"error":{}}}"#, json_str(&e)),
                ),
            }
        }
        ("GET", "/v1/mermaid") => {
            let path = query_param(query, "path");
            match parse_by_extension(&path) {
                Ok(g) => ("200 OK", "text/markdown; charset=utf-8", g.to_mermaid()),
                Err(e) => ("400 Bad Request", "text/plain", e.to_string()),
            }
        }
        ("GET", "/v1/layout") => {
            let path = query_param(query, "path");
            let nw: f32 = query
                .split('&')
                .find_map(|p| p.strip_prefix("w="))
                .and_then(|s| s.parse().ok())
                .unwrap_or(180.0);
            let nh: f32 = query
                .split('&')
                .find_map(|p| p.strip_prefix("h="))
                .and_then(|s| s.parse().ok())
                .unwrap_or(56.0);
            match parse_by_extension(&path) {
                Ok(g) => {
                    let lay = g.layout_layers(nw, nh, 48.0, 24.0);
                    let j = serde_json::json!({
                        "graph": g,
                        "layout": lay,
                    });
                    (
                        "200 OK",
                        "application/json",
                        serde_json::to_string_pretty(&j).unwrap_or_else(|_| "{}".into()),
                    )
                }
                Err(e) => (
                    "400 Bad Request",
                    "application/json",
                    format!(r#"{{"error":{}}}"#, json_str(&e.to_string())),
                ),
            }
        }
        _ => (
            "404 Not Found",
            "application/json",
            r#"{"error":"not found"}"#.into(),
        ),
    };

    let bytes = payload.as_bytes();
    let resp = format!(
        "HTTP/1.1 {status}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n",
        bytes.len()
    );
    stream.write_all(resp.as_bytes())?;
    stream.write_all(bytes)?;
    Ok(())
}

fn default_runs_root() -> PathBuf {
    dirs_home()
        .map(|h| h.join(".grok/sessions"))
        .unwrap_or_else(|| PathBuf::from("/home/johndpope/.grok/sessions"))
}

fn default_claude_log_path() -> PathBuf {
    dirs_home()
        .map(|h| h.join(".claude-workflows/runs.jsonl"))
        .unwrap_or_else(|| PathBuf::from("/home/johndpope/.claude-workflows/runs.jsonl"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Merge Grok session runs + Claude JSONL + shell pipeline STATUS into one payload.
/// Each run object is tagged with `backend: "grok"|"claude"|"pipeline"`.
fn list_all_runs(grok_root: &Path, claude_log: &Path) -> Result<String, String> {
    let mut runs: Vec<serde_json::Value> = Vec::new();
    for mut r in collect_grok_runs(grok_root) {
        if let Some(obj) = r.as_object_mut() {
            obj.insert("backend".into(), serde_json::Value::String("grok".into()));
        }
        runs.push(r);
    }
    for mut r in collect_claude_runs(claude_log) {
        if let Some(obj) = r.as_object_mut() {
            obj.insert("backend".into(), serde_json::Value::String("claude".into()));
        }
        runs.push(r);
    }
    for mut r in collect_pipeline_runs() {
        if let Some(obj) = r.as_object_mut() {
            obj.insert(
                "backend".into(),
                serde_json::Value::String("pipeline".into()),
            );
        }
        runs.push(r);
    }
    // Active first, then newest mtime.
    runs.sort_by(|a, b| {
        let rank = |s: &str| match s {
            "running" | "active" => 0,
            "paused" | "waiting" => 1,
            _ => 2,
        };
        let sa = a["status"].as_str().unwrap_or("");
        let sb = b["status"].as_str().unwrap_or("");
        let c = rank(sa).cmp(&rank(sb));
        if c != std::cmp::Ordering::Equal {
            return c;
        }
        let ma = a["mtime"].as_u64().unwrap_or(0);
        let mb = b["mtime"].as_u64().unwrap_or(0);
        mb.cmp(&ma)
    });
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "grok_root": grok_root.display().to_string(),
        "claude_log": claude_log.display().to_string(),
        "pipeline_root": default_rsrt_overnight_dir().display().to_string(),
        "runs": runs,
    }))
    .unwrap_or_else(|_| "{}".into()))
}

fn default_rsrt_overnight_dir() -> PathBuf {
    dirs_home()
        .map(|h| {
            h.join("Documents/GitHub/PresidentialDilema-FastApi/logs/rsrt_overnight")
        })
        .unwrap_or_else(|| {
            PathBuf::from(
                "/home/johndpope/Documents/GitHub/PresidentialDilema-FastApi/logs/rsrt_overnight",
            )
        })
}

/// Shell overnight pipelines that write STATUS.json (not Grok Rhai runs).
fn collect_pipeline_runs() -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    if let Some(r) = parse_rsrt_overnight_status(&default_rsrt_overnight_dir()) {
        out.push(r);
    }
    out
}

fn parse_rsrt_overnight_status(dir: &Path) -> Option<serde_json::Value> {
    let status_path = dir.join("STATUS.json");
    if !status_path.is_file() {
        return None;
    }
    let raw = std::fs::read_to_string(&status_path).ok()?;
    let st: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let mtime = std::fs::metadata(&status_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let finished = st.get("finished").and_then(|x| x.as_bool()).unwrap_or(false);
    let phase = st
        .get("phase")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let last = st
        .get("last")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let until = st
        .get("until")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let done = st.get("done").and_then(|x| x.as_u64()).unwrap_or(0);
    let queue = st.get("queue").and_then(|x| x.as_u64()).unwrap_or(0);
    let queue_left = st.get("queue_left").and_then(|x| x.as_u64()).unwrap_or(0);
    let cycle = st.get("cycle").and_then(|x| x.as_u64()).unwrap_or(0);
    let steps = st.get("steps").and_then(|x| x.as_u64()).unwrap_or(0);
    let n_new = st.get("n_new").and_then(|x| x.as_u64()).unwrap_or(0);
    let ckpt = st
        .get("ckpt")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let log_from_status = st
        .get("log")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();

    // Prefer pipeline.log.path (current stdout) over train sub-log.
    let log_path = {
        let ptr = dir.join("pipeline.log.path");
        if ptr.is_file() {
            std::fs::read_to_string(&ptr)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or(log_from_status)
        } else if !log_from_status.is_empty() {
            log_from_status
        } else {
            String::new()
        }
    };

    // Alive if STATUS says not finished AND pid file process still exists.
    let pid = {
        let p = dir.join("pipeline.pid");
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| s.trim().parse::<i32>().ok())
    };
    let pid_alive = pid.map(process_alive).unwrap_or(false);
    let status = if finished {
        "complete"
    } else if pid_alive || !phase.is_empty() {
        // STATUS still updating → treat as running even if pid check fails (restart race)
        "running"
    } else {
        "unknown"
    };

    let summary = format!(
        "done={done} queue={queue} left={queue_left} cycle={cycle} last={last}"
    );
    let objective = if until.is_empty() {
        summary.clone()
    } else {
        format!("{summary} until={until}")
    };

    let mut running_labels = Vec::new();
    let mut done_labels = Vec::new();
    if status == "running" && !phase.is_empty() {
        running_labels.push(phase.clone());
    } else if status == "complete" && !phase.is_empty() {
        done_labels.push(phase.clone());
    }

    Some(serde_json::json!({
        "run_id": "rsrt_overnight",
        "name": "rsrt-overnight",
        "script_name": "rsrt-overnight",
        "status": status,
        "current_phase": phase,
        "agents_used": 0,
        "agent_budget": 0,
        "pause_message": summary,
        "objective": objective,
        "state_path": status_path.display().to_string(),
        "log_path": log_path,
        "log_source": "rsrt_overnight",
        "mtime": mtime,
        "agents": [],
        "total_tokens": 0,
        "tokens_by_phase": [],
        "running_labels": running_labels,
        "done_labels": done_labels,
        "done_phases": [],
        "entered_phases": if phase.is_empty() { vec![] } else { vec![phase.clone()] },
        "pipeline": {
            "done": done,
            "queue": queue,
            "queue_left": queue_left,
            "cycle": cycle,
            "steps": steps,
            "n_new": n_new,
            "last": last,
            "until": until,
            "ckpt": ckpt,
            "pid": pid,
            "pid_alive": pid_alive,
            "finished": finished,
        },
    }))
}

fn process_alive(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // /proc is enough on Linux; no signal needed.
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Resolve an allowlisted log path and return a tail slice as JSON.
fn serve_log_tail(
    source: &str,
    path_q: &str,
    n_lines: usize,
    offset: Option<u64>,
) -> Result<String, String> {
    let path = resolve_log_path(source, path_q)?;
    let meta = std::fs::metadata(&path).map_err(|e| format!("stat: {e}"))?;
    let size = meta.len();

    // If offset given: read from that byte to EOF (cap ~512 KiB).
    // Else: last n_lines of file.
    let (start, text, truncated) = if let Some(off) = offset {
        let off = off.min(size);
        let max_bytes: u64 = 512 * 1024;
        let end = size;
        let slice_len = (end - off).min(max_bytes);
        let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        use std::io::{Read, Seek, SeekFrom};
        f.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; slice_len as usize];
        let nread = f.read(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(nread);
        let truncated = off + (nread as u64) < size && slice_len == max_bytes;
        let text = String::from_utf8_lossy(&buf).into_owned();
        (off, text, truncated)
    } else {
        // Tail last n_lines efficiently for large files.
        let (text, start_byte) = tail_file_lines(&path, n_lines, size)?;
        (start_byte, text, false)
    };

    let next_offset = start + text.len() as u64;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "path": path.display().to_string(),
        "source": if source.is_empty() { serde_json::Value::Null } else { serde_json::json!(source) },
        "size": size,
        "offset": start,
        "next_offset": next_offset.min(size),
        "n_lines": n_lines,
        "text": text,
        "truncated": truncated,
    }))
    .unwrap_or_else(|_| "{}".into()))
}

fn resolve_log_path(source: &str, path_q: &str) -> Result<PathBuf, String> {
    let candidate = if !source.is_empty() {
        match source {
            "rsrt_overnight" | "rsrt" | "overnight" => {
                let dir = default_rsrt_overnight_dir();
                // Prefer live pipeline pointer, then STATUS.log, then newest pipeline_*.log
                let ptr = dir.join("pipeline.log.path");
                if ptr.is_file() {
                    if let Ok(s) = std::fs::read_to_string(&ptr) {
                        let p = PathBuf::from(s.trim());
                        if p.is_file() {
                            p
                        } else {
                            newest_pipeline_log(&dir).ok_or_else(|| {
                                "rsrt_overnight: no pipeline log".to_string()
                            })?
                        }
                    } else {
                        newest_pipeline_log(&dir)
                            .ok_or_else(|| "rsrt_overnight: no pipeline log".to_string())?
                    }
                } else {
                    newest_pipeline_log(&dir)
                        .ok_or_else(|| "rsrt_overnight: no pipeline log".to_string())?
                }
            }
            other => return Err(format!("unknown log source: {other}")),
        }
    } else if !path_q.is_empty() {
        PathBuf::from(path_q)
    } else {
        return Err("missing source= or path=".into());
    };

    let abs = candidate
        .canonicalize()
        .map_err(|e| format!("path not found: {} ({e})", candidate.display()))?;
    if !path_is_allowlisted(&abs) {
        return Err(format!(
            "path not allowlisted: {} (allowed: rsrt_overnight logs, ~/.grok/sessions journals)",
            abs.display()
        ));
    }
    if !abs.is_file() {
        return Err(format!("not a file: {}", abs.display()));
    }
    Ok(abs)
}

fn newest_pipeline_log(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let rd = std::fs::read_dir(dir).ok()?;
    for ent in rd.flatten() {
        let p = ent.path();
        let name = p.file_name()?.to_str()?;
        if name.starts_with("pipeline_") && name.ends_with(".log") {
            let m = ent.metadata().ok()?.modified().ok()?;
            if best.as_ref().map(|(t, _)| m > *t).unwrap_or(true) {
                best = Some((m, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn path_is_allowlisted(abs: &Path) -> bool {
    let mut roots = Vec::new();
    roots.push(default_rsrt_overnight_dir());
    if let Some(h) = dirs_home() {
        roots.push(h.join(".grok/sessions"));
        roots.push(h.join("Documents/GitHub/PresidentialDilema-FastApi/logs"));
        roots.push(h.join("Documents/GitHub/adventure-engine/logs"));
    }
    for root in roots {
        let root_abs = root.canonicalize().unwrap_or(root);
        if abs.starts_with(&root_abs) {
            return true;
        }
    }
    false
}

fn tail_file_lines(path: &Path, n_lines: usize, size: u64) -> Result<(String, u64), String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    // Read last chunk (up to 256 KiB) and take last n lines.
    let max_chunk: u64 = 256 * 1024;
    let start = size.saturating_sub(max_chunk);
    f.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();
    let take = n_lines.min(lines.len());
    let slice = &lines[lines.len() - take..];
    // Approximate start offset: if we skipped a partial first line after start, next full line.
    let joined = slice.join("\n");
    let start_byte = if start == 0 {
        0
    } else {
        // We may have started mid-line; consumers using next_offset should still advance.
        size.saturating_sub(joined.len() as u64)
    };
    Ok((joined, start_byte))
}

fn collect_grok_runs(sessions_root: &Path) -> Vec<serde_json::Value> {
    if !sessions_root.is_dir() {
        return Vec::new();
    }
    let mut state_files = Vec::new();
    collect_state_json(sessions_root, 0, &mut state_files);
    let mut runs = Vec::new();
    for state_path in state_files {
        if let Some(run) = parse_run_state(&state_path) {
            runs.push(run);
        }
    }
    runs
}

/// Replay the append-only JSONL log the Python `_runtime.py` writes, grouping
/// events by `run_id` and folding them into the same run-snapshot shape the
/// Grok side emits. Non-JSONL lines are skipped silently.
fn collect_claude_runs(log_path: &Path) -> Vec<serde_json::Value> {
    let raw = match std::fs::read_to_string(log_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let mtime = std::fs::metadata(log_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Per-run accumulators.
    struct Agent {
        label: String,
        state: String,
        phase: String,
        tokens_used: u64,
        duration_ms: u64,
    }
    struct Run {
        run_id: String,
        name: String,
        entrypoint: String,
        cwd: String,
        status: String,
        current_phase: String,
        pause_message: String,
        entered_phases: Vec<String>,
        agents: Vec<Agent>,
    }
    let mut runs: std::collections::BTreeMap<String, Run> = std::collections::BTreeMap::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let run_id = v.get("run_id").and_then(|x| x.as_str()).unwrap_or("");
        if run_id.is_empty() {
            continue;
        }
        let event = v.get("event").and_then(|x| x.as_str()).unwrap_or("");
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let entry = runs.entry(run_id.to_string()).or_insert(Run {
            run_id: run_id.to_string(),
            name: name.clone(),
            entrypoint: String::new(),
            cwd: String::new(),
            status: "running".into(),
            current_phase: String::new(),
            pause_message: String::new(),
            entered_phases: Vec::new(),
            agents: Vec::new(),
        });
        if entry.name.is_empty() && !name.is_empty() {
            entry.name = name;
        }
        match event {
            "run_started" => {
                entry.entrypoint = v
                    .get("entrypoint")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                entry.cwd = v
                    .get("cwd")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                entry.status = "running".into();
            }
            "phase_entered" => {
                let phase = v
                    .get("phase")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if !phase.is_empty() {
                    entry.current_phase = phase.clone();
                    if !entry.entered_phases.iter().any(|p| p == &phase) {
                        entry.entered_phases.push(phase);
                    }
                }
            }
            "agent_started" => {
                let label = v
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let phase = v
                    .get("phase")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if !label.is_empty() {
                    entry.agents.push(Agent {
                        label,
                        state: "running".into(),
                        phase,
                        tokens_used: 0,
                        duration_ms: 0,
                    });
                }
            }
            "agent_completed" => {
                let label = v.get("label").and_then(|x| x.as_str()).unwrap_or("");
                let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
                let tokens = v
                    .get("tokens_used")
                    .and_then(|x| x.as_u64())
                    .unwrap_or(0);
                let dur = v.get("duration_ms").and_then(|x| x.as_u64()).unwrap_or(0);
                // Update the most recent matching Agent (agents can repeat via retries).
                if let Some(a) = entry.agents.iter_mut().rev().find(|a| a.label == label) {
                    a.state = if ok { "done".into() } else { "failed".into() };
                    a.tokens_used = tokens;
                    a.duration_ms = dur;
                }
            }
            "gate_waiting" => {
                entry.status = "paused".into();
                entry.pause_message = v
                    .get("message")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
            }
            "gate_released" => {
                if entry.status == "paused" {
                    entry.status = "running".into();
                }
                entry.pause_message.clear();
            }
            "run_completed" => {
                entry.status = v
                    .get("status")
                    .and_then(|x| x.as_str())
                    .unwrap_or("completed")
                    .to_string();
                entry.pause_message.clear();
            }
            _ => {}
        }
    }

    runs.into_values()
        .map(|r| {
            let running_labels: Vec<String> = r
                .agents
                .iter()
                .filter(|a| a.state == "running")
                .map(|a| a.label.clone())
                .collect();
            let done_labels: Vec<String> = r
                .agents
                .iter()
                .filter(|a| a.state == "done")
                .map(|a| a.label.clone())
                .collect();
            let mut done_phases: std::collections::BTreeSet<String> =
                std::collections::BTreeSet::new();
            for a in &r.agents {
                if a.state == "done" && !a.phase.is_empty() {
                    done_phases.insert(a.phase.clone());
                }
            }
            let mut phase_tokens: std::collections::BTreeMap<String, u64> =
                std::collections::BTreeMap::new();
            for a in &r.agents {
                let ph = if a.phase.is_empty() {
                    "(none)".to_string()
                } else {
                    a.phase.clone()
                };
                *phase_tokens.entry(ph).or_insert(0) += a.tokens_used;
            }
            let tokens_by_phase: Vec<serde_json::Value> = phase_tokens
                .into_iter()
                .map(|(phase, tokens)| serde_json::json!({ "phase": phase, "tokens": tokens }))
                .collect();
            let total_tokens: u64 = r.agents.iter().map(|a| a.tokens_used).sum();
            let agents_used = r.agents.len() as u64;
            let agent_list: Vec<serde_json::Value> = r
                .agents
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "label": a.label,
                        "state": a.state,
                        "phase": a.phase,
                        "tokens_used": a.tokens_used,
                        "duration_ms": a.duration_ms,
                    })
                })
                .collect();
            serde_json::json!({
                "run_id": r.run_id,
                "name": r.name.clone(),
                "script_name": r.name,
                "status": r.status,
                "current_phase": r.current_phase,
                "agents_used": agents_used,
                "agent_budget": 0,
                "pause_message": r.pause_message,
                "objective": r.entrypoint,
                "state_path": log_path.display().to_string(),
                "cwd": r.cwd,
                "mtime": mtime,
                "agents": agent_list,
                "total_tokens": total_tokens,
                "tokens_by_phase": tokens_by_phase,
                "running_labels": running_labels,
                "done_labels": done_labels,
                "done_phases": done_phases.into_iter().collect::<Vec<_>>(),
                "entered_phases": r.entered_phases,
            })
        })
        .collect()
}

fn collect_state_json(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    // Cap recursion: sessions/<cwd>/<sid>/workflows/<id>/state.json ≈ 4 levels
    if depth > 6 {
        return;
    }
    let rd = match std::fs::read_dir(dir) {
        Ok(d) => d,
        Err(_) => return,
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            // Fast path: if this dir is named workflows, only look one level for state.json
            if p.file_name().and_then(|n| n.to_str()) == Some("workflows") {
                if let Ok(wfs) = std::fs::read_dir(&p) {
                    for w in wfs.flatten() {
                        let state = w.path().join("state.json");
                        if state.is_file() {
                            out.push(state);
                        }
                    }
                }
            } else {
                collect_state_json(&p, depth + 1, out);
            }
        } else if p.file_name().and_then(|n| n.to_str()) == Some("state.json") {
            // only if parent looks like a workflow run dir
            if p.parent()
                .and_then(|par| par.parent())
                .and_then(|gp| gp.file_name())
                .and_then(|n| n.to_str())
                == Some("workflows")
            {
                out.push(p);
            }
        }
    }
}

fn parse_run_state(state_path: &Path) -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string(state_path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    // state may be nested under "state" or flat
    let st = v.get("state").cloned().unwrap_or(v);
    let name = st.get("name")?.as_str()?.to_string();
    if name.is_empty() {
        return None;
    }
    let status = st
        .get("status")
        .and_then(|x| x.as_str())
        .unwrap_or("unknown")
        .to_string();
    let phase = st
        .get("current_phase")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let agents_used = st
        .get("agents_used")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let budget = st
        .get("agent_budget")
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let pause_message = st
        .get("pause_message")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let objective = st
        .get("objective")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let run_id = st
        .get("run_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let mtime = std::fs::metadata(state_path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Per-agent snapshot for canvas highlight
    let mut agent_list = Vec::new();
    let mut running_labels = Vec::new();
    let mut done_labels = Vec::new();
    let mut done_phases = std::collections::BTreeSet::new();
    if let Some(arr) = st.get("agents").and_then(|a| a.as_array()) {
        for a in arr {
            let label = a
                .get("label")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let astate = a
                .get("state")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let aphase = a
                .get("phase")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            if label.is_empty() {
                continue;
            }
            match astate.as_str() {
                "running" | "active" | "starting" => running_labels.push(label.clone()),
                "done" | "completed" | "success" => {
                    done_labels.push(label.clone());
                    if !aphase.is_empty() {
                        done_phases.insert(aphase.clone());
                    }
                }
                _ => {}
            }
            let tokens = a
                .get("tokens_used")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            let duration_ms = a
                .get("duration_ms")
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            agent_list.push(serde_json::json!({
                "label": label,
                "state": astate,
                "phase": aphase,
                "tokens_used": tokens,
                "duration_ms": duration_ms,
            }));
        }
    }
    let total_tokens: u64 = agent_list
        .iter()
        .map(|a| a["tokens_used"].as_u64().unwrap_or(0))
        .sum();
    // Phase rollups for UI bars
    let mut phase_tokens: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for a in &agent_list {
        let ph = a["phase"].as_str().unwrap_or("(none)").to_string();
        let t = a["tokens_used"].as_u64().unwrap_or(0);
        *phase_tokens.entry(ph).or_insert(0) += t;
    }
    let tokens_by_phase: Vec<serde_json::Value> = phase_tokens
        .into_iter()
        .map(|(phase, tokens)| {
            serde_json::json!({ "phase": phase, "tokens": tokens })
        })
        .collect();

    // Phases entered from history (for completed-rail fill)
    let mut entered_phases = Vec::new();
    if let Some(hist) = st.get("history").and_then(|h| h.as_array()) {
        for ev in hist {
            if ev.get("event").and_then(|x| x.as_str()) == Some("phase_entered") {
                if let Some(d) = ev.get("detail").and_then(|x| x.as_str()) {
                    if !entered_phases.iter().any(|p: &String| p == d) {
                        entered_phases.push(d.to_string());
                    }
                }
            }
        }
    }

    // workflow "script name" often first segment before -N
    let script_name = name
        .rsplit_once('-')
        .and_then(|(base, suf)| {
            if suf.chars().all(|c| c.is_ascii_digit()) {
                Some(base.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| name.clone());

    Some(serde_json::json!({
        "run_id": run_id,
        "name": name,
        "script_name": script_name,
        "status": status,
        "current_phase": phase,
        "agents_used": agents_used,
        "agent_budget": budget,
        "pause_message": pause_message,
        "objective": objective,
        "state_path": state_path.display().to_string(),
        "mtime": mtime,
        "agents": agent_list,
        "total_tokens": total_tokens,
        "tokens_by_phase": tokens_by_phase,
        "running_labels": running_labels,
        "done_labels": done_labels,
        "done_phases": done_phases.into_iter().collect::<Vec<_>>(),
        "entered_phases": entered_phases,
    }))
}

fn list_workflows(dir: &Path) -> Result<String, String> {
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }
    let dir = dir
        .canonicalize()
        .unwrap_or_else(|_| dir.to_path_buf());
    let mut items = Vec::new();
    let mut rd = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    while let Some(Ok(ent)) = rd.next() {
        let p = ent.path();
        if p.extension().and_then(|e| e.to_str()) == Some("rhai") {
            let name = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let abs = p.canonicalize().unwrap_or(p);
            items.push(serde_json::json!({
                "name": name,
                "path": abs.display().to_string(),
                "backend": "grok",
            }));
        }
    }
    items.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "dir": dir.display().to_string(),
        "workflows": items,
    }))
    .unwrap_or_else(|_| "{}".into()))
}

/// Scan a repo root for BOTH `.grok/workflows/*.rhai` and `.claude/workflows/*.json`,
/// returning a merged list tagged with `backend` per entry.
fn list_workflows_repo(repo: &Path) -> Result<String, String> {
    if !repo.is_dir() {
        return Err(format!("not a repo root: {}", repo.display()));
    }
    let repo_abs = repo.canonicalize().unwrap_or_else(|_| repo.to_path_buf());
    let mut items = Vec::new();
    let grok_dir = repo_abs.join(".grok").join("workflows");
    if let Ok(rd) = std::fs::read_dir(&grok_dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().and_then(|e| e.to_str()) == Some("rhai") {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string();
                let abs = p.canonicalize().unwrap_or(p);
                items.push(serde_json::json!({
                    "name": name,
                    "path": abs.display().to_string(),
                    "backend": "grok",
                }));
            }
        }
    }
    let claude_dir = repo_abs.join(".claude").join("workflows");
    if let Ok(rd) = std::fs::read_dir(&claude_dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            if p.extension().and_then(|e| e.to_str()) == Some("json") {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("?")
                    .to_string();
                let abs = p.canonicalize().unwrap_or(p);
                items.push(serde_json::json!({
                    "name": name,
                    "path": abs.display().to_string(),
                    "backend": "claude",
                }));
            }
        }
    }
    items.sort_by(|a, b| {
        let ka = (
            a["backend"].as_str().unwrap_or(""),
            a["name"].as_str().unwrap_or(""),
        );
        let kb = (
            b["backend"].as_str().unwrap_or(""),
            b["name"].as_str().unwrap_or(""),
        );
        ka.cmp(&kb)
    });
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "repo": repo_abs.display().to_string(),
        "workflows": items,
    }))
    .unwrap_or_else(|_| "{}".into()))
}

/// Dispatch parse based on file extension (`.json` → Claude manifest, else Rhai).
fn parse_by_extension(
    path: &str,
) -> Result<WorkflowGraph, adventure_workflow_graph::WorkflowGraphError> {
    if Path::new(path).extension().and_then(|e| e.to_str()) == Some("json") {
        parse_workflow_manifest_file(path)
    } else {
        parse_workflow_file(path)
    }
}

fn split_query(target: &str) -> (&str, &str) {
    if let Some(i) = target.find('?') {
        (&target[..i], &target[i + 1..])
    } else {
        (target, "")
    }
}

fn query_param(query: &str, key: &str) -> String {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return percent_decode(v);
            }
        }
    }
    String::new()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = |c: u8| -> Option<u8> {
                    match c {
                        b'0'..=b'9' => Some(c - b'0'),
                        b'a'..=b'f' => Some(c - b'a' + 10),
                        b'A'..=b'F' => Some(c - b'A' + 10),
                        _ => None,
                    }
                };
                if let (Some(a), Some(b)) = (h(bytes[i + 1]), h(bytes[i + 2])) {
                    out.push((a << 4) | b);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}
