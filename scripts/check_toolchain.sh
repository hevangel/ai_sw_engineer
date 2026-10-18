#!/usr/bin/env bash
# Report which tools the pipeline needs and which are installed. Never installs anything.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
missing=0

check() {
    local name=$1 why=$2
    if command -v "$name" > /dev/null 2>&1; then
        printf '  ok       %-14s %s\n' "$name" "$("$name" --version 2>&1 | head -n1)"
    else
        printf '  MISSING  %-14s %s\n' "$name" "$why"
        missing=$((missing + 1))
    fi
}

echo "==> host tools"
check git "submodules"
check cargo "Rust build"
check rustup "nightly toolchain for -Zbuild-std"
check cmake "LLVM builds"
check ninja "LLVM builds"
check python3 "lit test runner"
check clang "building LLVM / llvm-mos"

echo "==> rust components"
if command -v rustup > /dev/null 2>&1; then
    rustup toolchain list | sed 's/^/  /'
    rustup component list --toolchain nightly --installed 2> /dev/null | grep -q rust-src \
        && echo "  ok       rust-src (nightly)" || echo "  MISSING  rust-src on nightly (rustup component add rust-src --toolchain nightly)"
fi

echo "==> submodules"
git -C "$ROOT" submodule status | sed 's/^/  /'

echo "==> ai_hw_engineer simulators"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
if [[ -d $HW/design ]]; then echo "  ok       $HW"; else echo "  MISSING  $HW (set AI_HW_ENGINEER)"; missing=$((missing + 1)); fi

echo "==> $missing missing"
