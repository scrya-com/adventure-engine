//! Deterministic playtest scoring for Cell Block C MVP.
//!
//! This is the **oracle** path (perfect knowledge). A Holo 3.1 (or other CUA)
//! path can produce the same [`PlaytestReport`] JSON by driving the window
//! and filling `tasks` from observed success/failure.
//! See `PLAYTEST.md` — prefer Hcompany Holo-3.1 over Fara-7B.
//!
//! ```bash
//! cargo run -p example-08-shawshank-pac -- --playtest /tmp/shawshank_score.json
//! ```

use std::path::Path;
use std::time::Instant;

/// One scored task in a playtest run.
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub id: &'static str,
    pub description: &'static str,
    pub passed: bool,
    pub steps: u32,
    pub detail: String,
}

/// Aggregate report written as JSON (no serde dep — hand-rolled).
#[derive(Debug)]
pub struct PlaytestReport {
    pub game: &'static str,
    pub mode: &'static str,
    pub elapsed_ms: u128,
    pub tasks: Vec<TaskResult>,
    pub rubric: RubricScores,
}

/// Human/agent-facing rubric (0–1 each).
#[derive(Debug, Clone, Copy)]
pub struct RubricScores {
    /// Spine quest completable end-to-end.
    pub quest_complete: f32,
    /// Gating works (can't skip Look / Talk order).
    pub gates_honest: f32,
    /// Enough affordances that a CUA *could* discover (chrome present).
    pub discoverability: f32,
    /// Mean of the above.
    pub overall: f32,
}

impl PlaytestReport {
    pub fn passed(&self) -> bool {
        self.tasks.iter().all(|t| t.passed) && self.rubric.overall >= 0.75
    }

    pub fn to_json(&self) -> String {
        let mut s = String::new();
        s.push_str("{\n");
        s.push_str(&format!("  \"game\": \"{}\",\n", self.game));
        s.push_str(&format!("  \"mode\": \"{}\",\n", self.mode));
        s.push_str(&format!("  \"elapsed_ms\": {},\n", self.elapsed_ms));
        s.push_str(&format!("  \"passed\": {},\n", self.passed()));
        s.push_str("  \"rubric\": {\n");
        s.push_str(&format!(
            "    \"quest_complete\": {:.3},\n",
            self.rubric.quest_complete
        ));
        s.push_str(&format!(
            "    \"gates_honest\": {:.3},\n",
            self.rubric.gates_honest
        ));
        s.push_str(&format!(
            "    \"discoverability\": {:.3},\n",
            self.rubric.discoverability
        ));
        s.push_str(&format!("    \"overall\": {:.3}\n", self.rubric.overall));
        s.push_str("  },\n");
        s.push_str("  \"tasks\": [\n");
        for (i, t) in self.tasks.iter().enumerate() {
            let comma = if i + 1 < self.tasks.len() { "," } else { "" };
            s.push_str("    {\n");
            s.push_str(&format!("      \"id\": \"{}\",\n", t.id));
            s.push_str(&format!(
                "      \"description\": \"{}\",\n",
                escape_json(t.description)
            ));
            s.push_str(&format!("      \"passed\": {},\n", t.passed));
            s.push_str(&format!("      \"steps\": {},\n", t.steps));
            s.push_str(&format!(
                "      \"detail\": \"{}\"\n",
                escape_json(&t.detail)
            ));
            s.push_str(&format!("    }}{comma}\n"));
        }
        s.push_str("  ],\n");
        s.push_str("  \"cua_notes\": \"Optional: run Holo-3.1 (Hcompany) against the windowed host with the same task ids; compare CUA success to oracle. Local: vllm serve Hcompany/Holo-3.1-4B. Not content gen.\"\n");
        s.push_str("}\n");
        s
    }

    pub fn write_to(&self, path: &Path) -> std::io::Result<()> {
        std::fs::write(path, self.to_json())
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// Build rubric from task results + chrome self-check flags.
pub fn score_rubric(tasks: &[TaskResult], chrome_ok: bool) -> RubricScores {
    let quest = if tasks.iter().all(|t| t.passed) {
        1.0
    } else {
        tasks.iter().filter(|t| t.passed).count() as f32 / tasks.len().max(1) as f32
    };
    let gates = tasks
        .iter()
        .filter(|t| t.id.starts_with("gate_"))
        .fold((0u32, 0u32), |(ok, n), t| {
            (ok + t.passed as u32, n + 1)
        });
    let gates_honest = if gates.1 == 0 {
        1.0
    } else {
        gates.0 as f32 / gates.1 as f32
    };
    let discoverability = if chrome_ok { 0.85 } else { 0.35 };
    let overall = (quest + gates_honest + discoverability) / 3.0;
    RubricScores {
        quest_complete: quest,
        gates_honest,
        discoverability,
        overall,
    }
}

/// Time a closure for report metadata.
pub fn timed<F: FnOnce() -> R, R>(f: F) -> (R, u128) {
    let t0 = Instant::now();
    let r = f();
    (r, t0.elapsed().as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_shape() {
        let r = PlaytestReport {
            game: "shawshank_cellblock",
            mode: "oracle",
            elapsed_ms: 12,
            tasks: vec![TaskResult {
                id: "t1",
                description: "demo",
                passed: true,
                steps: 1,
                detail: "ok".into(),
            }],
            rubric: RubricScores {
                quest_complete: 1.0,
                gates_honest: 1.0,
                discoverability: 0.85,
                overall: 0.95,
            },
        };
        let j = r.to_json();
        assert!(j.contains("\"passed\": true"));
        assert!(j.contains("cua_notes"));
    }
}
