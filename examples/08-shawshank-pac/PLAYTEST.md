# Cell Block C playtest + Fara-7B notes

## Oracle (deterministic — CI)

```bash
cargo test -p example-08-shawshank-pac
cargo run -p example-08-shawshank-pac -- --playtest /tmp/shawshank_score.json
```

Report JSON fields: `tasks[]`, `rubric.{quest_complete,gates_honest,discoverability,overall}`, `passed`.

## Windowed chrome

```bash
cargo run -p example-08-shawshank-pac
```

- **Status strip** (top): day + narration
- **Verb bar** (bottom): LOOK / TALK / USE + **NEXT DAY**
- **Hotspots**: bright glow; hover label; click with active verb
- Keys: `L`/`T`/`U` verb, `N` next day, `1`–`4` shortcuts, Esc quit

## Optional Fara-7B (computer-use scorer)

Fara is a **player/critic**, not content gen.

1. Run the windowed host.
2. Task list (same ids as oracle):
   - `gate_n_before_look` — force Next Day before Look; should fail/get feedback
   - `quest_cellblock_spine` — complete spine with **mouse only** if possible
3. Capture screenshots + action log.
4. Emit the same JSON shape as `--playtest` with `mode: "fara-7b"`.
5. Gate CI: `overall >= 0.75` and `quest_cellblock_spine.passed`.

Local serve example (see Microsoft Fara docs):

```bash
# vllm / LM Studio endpoint — project-specific
# fara-cli --fara-7b --task "In the Shawshank window, look at Red's cell then advance the day"
```

Do **not** burn Imagine/media budget on playtest scoring.
