#!/usr/bin/env bash
# Run the llvm-i4004 MC lit/FileCheck tests.
#
# Runs inside the ai-sw-engineer container (has python3 + lit). Tools come from
# llvm-i4004/install/bin, built by scripts/build_llvm.sh.
#
#   docker run --rm -v B:\ai_sw_engineer_mc:/workspace -w /workspace \
#       ai-sw-engineer:latest bash -lc 'llvm-i4004/scripts/run_tests.sh'
set -euo pipefail

BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$BACKEND_DIR/install/bin"
LIT="${LIT:-lit}"

[[ -x $BIN/llvm-mc ]] || {
  echo "ERROR: $BIN/llvm-mc missing; run scripts/build_llvm.sh first" >&2
  exit 1
}

export PATH="$BIN:$PATH"
echo "==> llvm-mc: $(llvm-mc --version | head -n1)"
echo "==> running lit on tests/mc"
exec "$LIT" -v "$BACKEND_DIR/tests/mc"
