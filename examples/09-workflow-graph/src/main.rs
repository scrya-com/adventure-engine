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
//! ```

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

use adventure_workflow_graph::{parse_workflow, parse_workflow_file, WorkflowGraph};

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

    let graph = parse_workflow_file(&path).unwrap_or_else(|e| {
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
           GET  /v1/workflows?dir=/path/to/.grok/workflows\n\
           GET  /v1/parse?path=/abs/file.rhai\n\
           POST /v1/parse     body = rhai source text\n\
           GET  /v1/mermaid?path=…\n\
           GET  /v1/layout?path=…&w=180&h=56\n"
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
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("bind {addr}: {e}");
        std::process::exit(1);
    });
    eprintln!("workflow-graph API  http://{addr}");
    eprintln!("  GET /health");
    eprintln!("  GET /v1/workflows?dir=…");
    eprintln!("  GET /v1/parse?path=…");
    eprintln!("  POST /v1/parse  (body = rhai source)");
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
            let dir_s = query_param(query, "dir");
            let dir = if dir_s.is_empty() {
                default_workflows_dir()
            } else {
                PathBuf::from(dir_s)
            };
            match list_workflows(&dir) {
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
                match parse_workflow_file(&path) {
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
        ("POST", "/v1/parse") => match parse_workflow(&body) {
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
        },
        ("GET", "/v1/mermaid") => {
            let path = query_param(query, "path");
            match parse_workflow_file(&path) {
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
            match parse_workflow_file(&path) {
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
