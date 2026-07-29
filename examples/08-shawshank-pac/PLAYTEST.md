# Cell Block C playtest + Holo 3.1 (computer-use scorer)

**CUA scorer:** [Holo 3.1](https://huggingface.co/collections/Hcompany/holo31) (H Company), **not** Fara-7B.  
Oracle path stays deterministic; Holo is the optional vision/mouse player-critic.

## Oracle (deterministic — CI)

```bash
cargo test -p example-08-shawshank-pac
cargo run -p example-08-shawshank-pac -- --playtest /tmp/shawshank_score.json
```

Report JSON: `tasks[]`, `rubric.{quest_complete,gates_honest,discoverability,overall}`, `passed`.

## Windowed chrome

```bash
cargo run -p example-08-shawshank-pac
```

- Status strip · LOOK / TALK / USE · NEXT DAY  
- Bright hotspots + hover labels · dialog text  

## Holo 3.1 instead of Fara

Holo is a **VLM computer-use family** (web / desktop / mobile). Use it as:

1. **Player** — screenshot window → model proposes click/type → harness applies  
2. **Scorer** — same task ids as oracle; emit `mode: "holo-3.1"` JSON  

Not content gen / not Imagine.

### Model pick (this MSI: ~24 GB VRAM)

| Model | HF id | Fit |
| --- | --- | --- |
| **Holo-3.1-4B** (recommended start) | `Hcompany/Holo-3.1-4B` | Comfortable on 24 GB BF16/FP8 |
| Holo-3.1-0.8B | `Hcompany/Holo-3.1-0.8B` | Edge / fastest smoke |
| Holo-3.1-9B | `Hcompany/Holo-3.1-9B` | Possible if quantized |
| Holo-3.1-35B-A3B | GGUF / FP8 / NVFP4 | Heavy; prefer quant on 24 GB |

Collection: https://huggingface.co/collections/Hcompany/holo31  
Official local serve: https://hub.hcompany.ai/holo-desktop-cli/how-to/run-a-local-model-server  

**Disk:** weights need free space under `~/.cache/huggingface` (this machine was near full — free space before download).

### Spin up locally (vLLM — you already have `vllm` on MSI)

```bash
# Recommended: 4B computer-use VLM
export HF_HOME="${HF_HOME:-$HOME/.cache/huggingface}"

vllm serve Hcompany/Holo-3.1-4B \
  --served-model-name holo3-1-4b \
  --host 0.0.0.0 \
  --port 8008 \
  --dtype auto \
  --gpu-memory-utilization 0.85 \
  --max-model-len 8192 \
  --limit-mm-per-prompt '{"image": 2, "video": 0}'
```

Smoke (OpenAI-compatible):

```bash
curl -s http://127.0.0.1:8008/v1/models | head -c 400
```

Optional: [HoloDesktop CLI](https://hub.hcompany.ai/holo-desktop-cli/getting-started/quickstart) pointed at the server:

```bash
# after installing holo CLI per H docs
holo run "Focus the Shawshank PAC window. Select LOOK and click Red's cell." \
  --base-url http://127.0.0.1:8008/v1 \
  --model holo3-1-4b
```

Hosted API alternative (no local weights): `https://api.hcompany.ai/v1/` + `HAI_API_KEY` — see https://hub.hcompany.ai/quickstart

### Task list (same ids as oracle)

| id | Goal |
| --- | --- |
| `gate_n_before_look` | NEXT DAY before Look → should fail / get feedback |
| `quest_cellblock_spine` | LOOK Red → NEXT DAY → LOOK Andy → TALK → USE stone |
| `chrome_layout` | Status + verb bar visible (screenshot check) |

1. Start `cargo run -p example-08-shawshank-pac`  
2. Capture screenshots + action log from Holo loop  
3. Emit report with `mode: "holo-3.1"` matching `--playtest` shape  
4. Gate: `overall >= 0.75` and `quest_cellblock_spine.passed`

### Wire to Shawshank playtest JSON

Oracle already writes `cua_notes` describing Holo. A thin harness (future) can:

```text
screenshot → Holo chat.completions (image + task) → click coords
  → inject into window / OS → re-screenshot → until done
  → write /tmp/shawshank_score_holo.json
```

Do **not** spend Imagine/media budget on playtest scoring.

## What’s next (product floor)

Chrome is in PR `feat/example-08-playable-chrome`. After that: AVAL 8-dir body so Holo has a character to “see,” not only a hallway plate.
