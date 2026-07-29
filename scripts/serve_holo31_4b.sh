#!/usr/bin/env bash
# Serve Hcompany Holo-3.1-4B locally (computer-use VLM) via vLLM.
# Prefer this over Fara-7B for Shawshank PAC playtest scoring.
#
# Needs: vllm, ~10–12 GB VRAM free (RTX 24 GB class), disk under HF cache.
# Docs: https://hub.hcompany.ai/holo-desktop-cli/how-to/run-a-local-model-server
# Weights: https://huggingface.co/Hcompany/Holo-3.1-4B
set -euo pipefail

MODEL="${HOLO_MODEL:-Hcompany/Holo-3.1-4B}"
NAME="${HOLO_SERVED_NAME:-holo3-1-4b}"
PORT="${HOLO_PORT:-8008}"
UTIL="${HOLO_GPU_UTIL:-0.85}"
MAXLEN="${HOLO_MAX_LEN:-8192}"

if ! command -v vllm >/dev/null 2>&1; then
  echo "vllm not on PATH — install or activate the env that has it" >&2
  exit 1
fi

# Rough free-space check (HF download is multi‑GB)
CACHE="${HF_HOME:-$HOME/.cache/huggingface}"
mkdir -p "$CACHE"
avail_kb=$(df -Pk "$CACHE" | awk 'NR==2{print $4}')
if [[ "${avail_kb:-0}" -lt 15000000 ]]; then
  echo "WARN: <~15 GB free under $CACHE — download may fail (df -h $CACHE)" >&2
fi

echo "Serving $MODEL as $NAME on :$PORT"
exec vllm serve "$MODEL" \
  --served-model-name "$NAME" \
  --host 0.0.0.0 \
  --port "$PORT" \
  --dtype auto \
  --gpu-memory-utilization "$UTIL" \
  --max-model-len "$MAXLEN" \
  --limit-mm-per-prompt '{"image": 2, "video": 0}'
