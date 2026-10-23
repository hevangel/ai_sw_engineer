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

# Prefer the installed tools; fall back to the shared LLVM build volume used by
# scripts/build_llvm.sh (ai-sw-llvm-build mounted at /llvm-build) when the repo
# has no local install/ yet.
if [[ ! -x $BIN/llvm-mc ]]; then
  if [[ -x /llvm-build/llvm-obj/bin/llvm-mc ]]; then
    BIN="/llvm-build/llvm-obj/bin"
  fi
fi

[[ -x $BIN/llvm-mc ]] || {
  echo "ERROR: llvm-mc not found in $BACKEND_DIR/install/bin or " \
       "/llvm-build/llvm-obj/bin; run scripts/build_llvm.sh first" >&2
  exit 1
}

export PATH="$BIN:$PATH"
echo "==> llvm-mc: $(llvm-mc --version | head -n1)"
echo "==> running lit on tests/mc"
"$LIT" -v "$BACKEND_DIR/tests/mc"

echo
echo "==> running lit on tests/codegen (llc selection-pattern FileCheck tests)"
if [[ -x $BIN/llc ]]; then
  "$LIT" -v "$BACKEND_DIR/tests/codegen"
else
  echo "    (skipped: llc not built -- run build_llvm.sh with the CodeGen target)"
fi

echo
echo "==> differential disassembler cross-check (executes every encoding on external/chips)"
LLVM_MC="$BIN/llvm-mc" python3 "$BACKEND_DIR/scripts/crosscheck_disasm.py"

echo
echo "==> CodeGen execution test: run llc-compiled functions on external/chips"
if [[ -x $BIN/llc ]]; then
  LLC="$BIN/llc" LLVM_MC="$BIN/llvm-mc" LLVM_OBJDUMP="$BIN/llvm-objdump" \
    python3 "$BACKEND_DIR/scripts/exec_fn.py"
  LLC="$BIN/llc" LLVM_MC="$BIN/llvm-mc" LLVM_OBJDUMP="$BIN/llvm-objdump" \
    python3 "$BACKEND_DIR/scripts/exec_i16.py"
  LLC="$BIN/llc" LLVM_MC="$BIN/llvm-mc" LLVM_OBJDUMP="$BIN/llvm-objdump" \
    python3 "$BACKEND_DIR/scripts/exec_cmp.py"
  LLC="$BIN/llc" LLVM_MC="$BIN/llvm-mc" LLVM_OBJDUMP="$BIN/llvm-objdump" \
    python3 "$BACKEND_DIR/scripts/exec_nibble.py"
else
  echo "    (skipped: llc not built)"
fi

echo
echo "==> whole-firmware size gate (busicom_141pf; ROM_BUDGET=${ROM_BUDGET:-4096}, FN_MAX=${FN_MAX:-1024})"
# Fails the run (exit 3) while the firmware does not fit -- by design: the
# 2026-10-19 failure note's lesson is that every component was green while the
# whole could not be built. SIZE_GATE=report downgrades it to a report.
FW_IR="$BACKEND_DIR/../system/busicom_141pf/firmware/build/firmware_i4004.ll"
if [[ -x $BIN/llc && -f $FW_IR ]]; then
  LLVM_OBJDUMP="$BIN/llvm-objdump" python3 \
    "$BACKEND_DIR/../system/busicom_141pf/firmware/scripts/gen_worklist.py" \
    "$FW_IR" "$BIN/llc" "$BACKEND_DIR/rust/codegen_worklist.md"
else
  echo "    (skipped: no llc or no firmware IR -- run build_target_ir.sh)"
fi
