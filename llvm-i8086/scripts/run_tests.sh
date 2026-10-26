#!/usr/bin/env bash
# Run the llvm-i8086 lit suites (MC encodings, disassembly, relaxation,
# relocations, diagnostics) and, when the hardware project's physical-chip
# vector corpus is available, the corpus check. Inside the ai-sw-engineer
# container:
#   docker run --rm -v <repo>:/workspace -v <ai_hw_engineer>:/hw:ro \
#     -w /workspace ai-sw-engineer:latest llvm-i8086/scripts/run_tests.sh
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIT="${LIT:-lit}"
BIN="$BACKEND_DIR/install/bin"

[[ -x $BIN/llvm-mc ]] || { echo "ERROR: $BIN/llvm-mc missing; run scripts/build_llvm.sh" >&2; exit 1; }

echo "==> regenerate tests/mc/encodings.s from the manual's opcode table"
if command -v node >/dev/null; then
  node "$BACKEND_DIR/tests/mc/gen_encodings.js"
else
  echo "    (node not installed: using the committed tests/mc/encodings.s)"
fi

echo "==> lit: tests/mc, tests/codegen"
I8086_TEST_TMP="${I8086_TEST_TMP:-/tmp/i8086-lit}" PATH="$BIN:$PATH" \
  "$LIT" -sv "$BACKEND_DIR/tests"

echo "==> committed tests/run objects are current, and llc text == llc object"
"$BACKEND_DIR/scripts/gen_run_objects.sh" --check

echo "==> committed rt/obj objects are current"
"$BACKEND_DIR/scripts/build_rt.sh" --check

VECTORS="${I8086_VECTORS:-/hw/design/intel_8086/work/reference/v1}"
if [[ -f $VECTORS/metadata.json ]]; then
  echo "==> physical-chip vector corpus check ($VECTORS)"
  python3 "$BACKEND_DIR/tests/mc/vector_check.py" "$BIN" "$VECTORS" "${I8086_VECTOR_CASES:-30}"
else
  echo "==> skipped the corpus check: no vectors at $VECTORS (set I8086_VECTORS)"
fi
echo "==> llvm-i8086 tests passed (the execution tests run on the host: cd system/ibm_pc/emulator && cargo test --test i8086_codegen)"
