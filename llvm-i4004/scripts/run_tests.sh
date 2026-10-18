#!/usr/bin/env bash
# Run lit/FileCheck codegen + MC tests and simulator smoke programs.
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIT="${LIT:-lit}"
BIN="$BACKEND_DIR/install/bin"

[[ -x $BIN/llc ]] || { echo "ERROR: $BIN/llc missing; run scripts/build_llvm.sh" >&2; exit 1; }
PATH="$BIN:$PATH" "$LIT" -v "$BACKEND_DIR/tests/codegen" "$BACKEND_DIR/tests/mc"
