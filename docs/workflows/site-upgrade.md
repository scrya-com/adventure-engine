# site-upgrade

Graph for multi-surface Scrya upgrades that improve creator velocity, content reliability, discovery, and cross-surface consistency. Parallel specialized explore → prioritized plan → optional worktree implement → adversarial + journey verify → decision-ready report.

**When:** Any change that touches studio.scrya.com, platform.scrya.com, or api.scrya.com when the goal is higher engagement, better pack/challenge/media reliability, or stronger network effects. Not for single-file nits.

## Stats

| metric | count |
| --- | --- |
| phases | 5 |
| phase() | 5 |
| agent() | 2 |
| parallel() | 3 |
| complete() | 1 |
| gates | 4 |

## Graph

```mermaid
flowchart TD
  classDef phase fill:#2a3340,stroke:#c4a574,color:#e7e9ea
  classDef agent fill:#1a3a2a,stroke:#3d9a6a,color:#e7e9ea
  classDef parallel fill:#1a2a3a,stroke:#5b9fd4,color:#e7e9ea
  classDef gate fill:#3a2a1a,stroke:#c45c5c,color:#e7e9ea
  classDef start fill:#141a22,stroke:#8b98a5,color:#e7e9ea
  start([site-upgrade])
  class start start
  phase_0[Explore]
  class phase_0 phase
  parallel_0{{parallel×2}}
  class parallel_0 parallel
  gate_0[/pause/]
  class gate_0 gate
  gate_1[/pause/]
  class gate_1 gate
  phase_1[Plan]
  class phase_1 phase
  agent_0(plan)
  class agent_0 agent
  gate_2[/pause/]
  class gate_2 gate
  gate_3[/pause/]
  class gate_3 gate
  phase_2[Implement]
  class phase_2 phase
  parallel_1{{parallel×1}}
  class parallel_1 parallel
  phase_3[Verify]
  class phase_3 phase
  parallel_2{{parallel×2}}
  class parallel_2 parallel
  agent_1(scorecard)
  class agent_1 agent
  phase_4[Report]
  class phase_4 phase
  other_0(write_scratch)
  class other_0 start
  complete_0([complete])
  class complete_0 start
  start --> phase_0
  phase_0 -->|barrier| parallel_0
  parallel_0 -->|pause| gate_0
  gate_0 -->|pause| gate_1
  gate_1 --> phase_1
  phase_1 --> agent_0
  agent_0 -->|pause| gate_2
  gate_2 -->|pause| gate_3
  gate_3 --> phase_2
  phase_2 -->|barrier| parallel_1
  parallel_1 --> phase_3
  phase_3 -->|barrier| parallel_2
  parallel_2 --> agent_1
  agent_1 --> phase_4
  phase_4 --> other_0
  other_0 --> complete_0
```
