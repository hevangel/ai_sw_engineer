#!/usr/bin/env bash
# Run the llvm-i8008 lit suites (MC encodings, disassembly, codegen). Inside
# the ai-sw-engineer container (exec_tests.py needs the ai-hw-engineer image):
#   llvm-i8008/scripts/run_tests.sh
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$BACKEND_DIR/install/bin"
LIT="${LIT:-lit}"

[[ -x $BIN/llc ]] || { echo "ERROR: $BIN/llc missing; run scripts/build_llvm.sh" >&2; exit 1; }

echo "==> regenerate tests/mc/encodings.s from the MCS-8 table"
python3 "$BACKEND_DIR/tests/mc/gen_encodings.py"

echo "==> lit: tests/mc tests/codegen"
I8008_TEST_TMP="${I8008_TEST_TMP:-/tmp/i8008-lit}" PATH="$BIN:$PATH" \
  "$LIT" -sv "$BACKEND_DIR/tests/mc" "$BACKEND_DIR/tests/codegen"

echo "==> lit suites passed"
echo "    execution tests run on the RTL in the ai-hw-engineer image:"
echo "    python3 llvm-i8008/scripts/exec_tests.py   (plans/testplan.md)"
