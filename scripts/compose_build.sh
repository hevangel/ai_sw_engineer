#!/usr/bin/env bash
# Build the full toolchain image with docker compose, pinning LLVM to the
# external/llvm-project submodule commit recorded in git (never a guess).
# Usage: scripts/compose_build.sh [extra `docker compose build` args, e.g. --no-cache]
#   JOBS=16 scripts/compose_build.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pin="$(git -C "$ROOT" ls-tree HEAD external/llvm-project | awk '{print $3}')"
[[ -n $pin ]] || { echo "ERROR: external/llvm-project pin not found in git" >&2; exit 1; }
if [[ -n ${LLVM_PROJECT_COMMIT:-} && $LLVM_PROJECT_COMMIT != "$pin" ]]; then
    echo "ERROR: LLVM_PROJECT_COMMIT=$LLVM_PROJECT_COMMIT differs from the submodule pin $pin" >&2
    exit 1
fi
default="$(sed -n 's/.*LLVM_PROJECT_COMMIT:-\([0-9a-f]\{40\}\)}.*/\1/p' "$ROOT/docker-compose.yml")"
if [[ $default != "$pin" ]]; then
    echo "WARNING: docker-compose.yml default LLVM_PROJECT_COMMIT ($default) is stale; using the pin $pin" >&2
fi
export LLVM_PROJECT_COMMIT="$pin"
echo "==> building ai-sw-engineer:full (llvm-project $pin, JOBS=${JOBS:-8})"
exec docker compose -f "$ROOT/docker-compose.yml" build "$@" toolchain
