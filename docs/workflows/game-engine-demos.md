# game-engine-demos

Graph for Scrya adventure stack: NCP storyforms, adventure-engine (Ariadne) runtime, scene-engine locomotion, Flutter Scene sister, demos. Explore → plan → optional worktree implement → cargo/NCP/demo verify → report.

**When:** Upgrade game engine + demos, Shawshank PAC, NCP→RON pipeline, adventure-engine examples, or Dart Scene parity — not single-line widget tweaks.

## Stats

| metric | count |
| --- | --- |
| phases | 5 |
| phase() | 5 |
| agent() | 1 |
| parallel() | 3 |
| complete() | 1 |
| gates | 2 |

## Graph

```mermaid
flowchart TD
  classDef phase fill:#2a3340,stroke:#c4a574,color:#e7e9ea
  classDef agent fill:#1a3a2a,stroke:#3d9a6a,color:#e7e9ea
  classDef parallel fill:#1a2a3a,stroke:#5b9fd4,color:#e7e9ea
  classDef gate fill:#3a2a1a,stroke:#c45c5c,color:#e7e9ea
  classDef start fill:#141a22,stroke:#8b98a5,color:#e7e9ea
  start([game-engine-demos])
  class start start
  phase_0[Explore]
  class phase_0 phase
  parallel_0{{parallel×1}}
  class parallel_0 parallel
  gate_0[/pause/]
  class gate_0 gate
  phase_1[Plan]
  class phase_1 phase
  agent_0(plan)
  class agent_0 agent
  gate_1[/pause/]
  class gate_1 gate
  phase_2[Implement]
  class phase_2 phase
  parallel_1{{parallel×1}}
  class parallel_1 parallel
  phase_3[Verify]
  class phase_3 phase
  parallel_2{{parallel×3}}
  class parallel_2 parallel
  phase_4[Report]
  class phase_4 phase
  other_0(write_scratch)
  class other_0 start
  complete_0([complete])
  class complete_0 start
  start --> phase_0
  phase_0 -->|barrier| parallel_0
  parallel_0 -->|pause| gate_0
  gate_0 --> phase_1
  phase_1 --> agent_0
  agent_0 -->|pause| gate_1
  gate_1 --> phase_2
  phase_2 -->|barrier| parallel_1
  parallel_1 --> phase_3
  phase_3 -->|barrier| parallel_2
  parallel_2 --> phase_4
  phase_4 --> other_0
  other_0 --> complete_0
```
