#!/usr/bin/env bash
# Serve Hcompany Holo-3.1-4B locally (computer-use VLM) via vLLM.
# Prefer this over Fara-7B for Shawshank PAC playtest scoring.
#
# Needs: vllm, ~10–12 GB VRAM free (RTX 24 GB class), disk under HF cache.
# Docs: https://hub.hcompany.ai/holo-desktop-cli/how-to/run-a-local-model-server
# Weights: https://huggingface.co/Hcompany/Holo-3.1-4B
#
# Env:
#   HOLO_SKIP_KILL=1   — do not kill existing vllm (fail if port busy)
#   HOLO_PORT=8008     — listen port (also used to free listeners)
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

kill_existing_vllm() {
  if [[ "${HOLO_SKIP_KILL:-0}" == "1" ]]; then
    echo "HOLO_SKIP_KILL=1 — leaving existing vllm alone"
    return 0
  fi

  local pids=()
  # Anything that looks like a vLLM OpenAI server
  while read -r pid; do
    [[ -n "$pid" ]] && pids+=("$pid")
  done < <(pgrep -f '[v]llm serve|[v]llm\.entrypoints|[v]llm_openai' 2>/dev/null || true)

  # Also free this port (fuser / lsof if available)
  if command -v fuser >/dev/null 2>&1; then
    fuser -k "${PORT}/tcp" 2>/dev/null || true
  elif command -v lsof >/dev/null 2>&1; then
    while read -r pid; do
      [[ -n "$pid" ]] && pids+=("$pid")
    done < <(lsof -t -iTCP:"$PORT" -sTCP:LISTEN 2>/dev/null || true)
  fi

  if [[ ${#pids[@]} -eq 0 ]]; then
    echo "No existing vllm process found"
    return 0
  fi

  # Unique PIDs (sort -u)
  local uniq
  uniq=$(printf '%s\n' "${pids[@]}" | sort -u | tr '\n' ' ')
  # shellcheck disable=SC2086
  set -- $uniq
  if [[ $# -eq 0 ]]; then
    echo "No existing vllm process found"
    return 0
  fi

  echo "Stopping existing vllm (pids: $*)…"
  kill -TERM "$@" 2>/dev/null || true
  local i pid
  for i in 1 2 3 4 5 6 7 8 9 10; do
    local alive=0
    for pid in "$@"; do
      if kill -0 "$pid" 2>/dev/null; then
        alive=1
        break
      fi
    done
    if [[ "$alive" -eq 0 ]]; then
      break
    fi
    sleep 0.5
  done
  # Still alive → SIGKILL
  for pid in "$@"; do
    if kill -0 "$pid" 2>/dev/null; then
      echo "  SIGKILL $pid"
      kill -KILL "$pid" 2>/dev/null || true
    fi
  done
  sleep 0.5
  echo "Previous vllm cleared"
}

kill_existing_vllm

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
