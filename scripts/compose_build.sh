#!/usr/bin/env bash
# Build the full toolchain image with docker compose, pinning LLVM, llvm-mos and
# llvm-mos-sdk to the submodule commits recorded in git (never a guess).
# Usage: scripts/compose_build.sh [extra `docker compose build` args, e.g. --no-cache]
#   JOBS=16 scripts/compose_build.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pin_of() { git -C "$ROOT" ls-tree HEAD "external/$1" | awk '{print $3}'; }
for spec in LLVM_PROJECT_COMMIT:llvm-project LLVM_MOS_COMMIT:llvm-mos LLVM_MOS_SDK_COMMIT:llvm-mos-sdk; do
    var="${spec%%:*}"; sub="${spec#*:}"
    pin="$(pin_of "$sub")"
    [[ -n $pin ]] || { echo "ERROR: external/$sub pin not found in git" >&2; exit 1; }
    if [[ -n ${!var:-} && ${!var} != "$pin" ]]; then
        echo "ERROR: $var=${!var} differs from the external/$sub pin $pin" >&2
        exit 1
    fi
    default="$(sed -n "s/.*$var:-\([0-9a-f]\{40\}\)}.*/\1/p" "$ROOT/docker-compose.yml")"
    [[ $default == "$pin" ]] || \
        echo "WARNING: docker-compose.yml default $var ($default) is stale; using the pin $pin" >&2
    export "$var=$pin"
    echo "==> external/$sub $pin"
done
echo "==> building ai-sw-engineer:full (JOBS=${JOBS:-8})"
exec docker compose -f "$ROOT/docker-compose.yml" build "$@" toolchain
