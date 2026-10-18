#!/usr/bin/env bash
# Run a command (default: interactive bash) in the ai-sw-engineer toolchain container.
# The repo is mounted at /workspace and ai_hw_engineer (if present) at /ai_hw_engineer.
# Usage: ./scripts/docker_run.sh [command...]
#   e.g. ./scripts/docker_run.sh system/busicom_141pf/firmware/scripts/run_all.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${IMAGE:-ai-sw-engineer:latest}"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"

args=(--rm -v "$ROOT:/workspace" -w /workspace)
[[ -t 0 ]] && args+=(-it)
if [[ -d $HW ]]; then
    args+=(-v "$(cd "$HW" && pwd):/ai_hw_engineer")
else
    echo "NOTE: $HW not found; hardware simulators unavailable in the container" >&2
fi

if [[ $# -eq 0 ]]; then
    exec docker run "${args[@]}" "$IMAGE" bash
fi
exec docker run "${args[@]}" "$IMAGE" bash -lc "$(printf '%q ' "$@")"
