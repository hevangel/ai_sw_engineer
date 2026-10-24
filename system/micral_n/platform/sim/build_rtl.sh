#!/usr/bin/env bash
# Build micral_rtl: the ai_hw_engineer 8008 RTL (Verilator) + board.h.
# Runs in the ai-hw-engineer image with both repos mounted:
#   docker run --rm -v <ai_sw_engineer>:/work/ai_sw_engineer \
#     -v <ai_hw_engineer>:/work/ai_hw_engineer -w /work/ai_sw_engineer \
#     ai-hw-engineer:latest system/micral_n/platform/sim/build_rtl.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
hw="${AI_HW_ENGINEER:-$here/../../../../../ai_hw_engineer}"
rtl="$hw/design/intel_8008/src/intel_8008.sv"
out="${OUT:-$here/build}"
[[ -f $rtl ]] || { echo "ERROR: $rtl not found (set AI_HW_ENGINEER)" >&2; exit 1; }
echo "==> verilating $rtl"
mkdir -p "$out"
verilator --cc --exe --build -O3 -j 8 --top-module intel_8008 \
  -Wno-fatal -Wno-WIDTHEXPAND -Wno-WIDTHTRUNC \
  --Mdir "$out/obj" -CFLAGS "-O2 -I$here" -o micral_rtl \
  "$rtl" "$here/rtl_main.cpp" >"$out/verilator.log" 2>&1 || { tail -30 "$out/verilator.log"; exit 1; }
cp "$out/obj/micral_rtl" "$out/micral_rtl"
echo "==> $out/micral_rtl"
