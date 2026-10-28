#!/usr/bin/env bash
# Differential-fuzz the Rust 8088 core against the hardware project's 8086 RTL.
#
# Generates vectors with the core (examples/rtl_fuzz.rs), builds a Verilator
# runner from the unmodified RTL plus rtl/tb_diff.cpp inside the project's
# Docker image, and runs every vector file. Every disagreement is printed.
#
# Usage: scripts/rtl_diff.sh [cases_per_encoding] [seed]
# Needs: docker with image ai-hw-engineer:latest, a checkout of ai_hw_engineer
# ($AI_HW_ENGINEER, default ../ai_hw_engineer) with its vector corpus cached,
# and Rust on the host.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
HW="$(cd "$HW" && pwd)"
CASES="${1:-200}"
SEED="${2:-1}"
IMAGE="${DOCKER_IMAGE:-ai-hw-engineer:latest}"
OUT="$HERE/build"

[[ -d $HW/design/intel_8086/src ]] || { echo "ERROR: $HW/design/intel_8086 not found" >&2; exit 1; }
mkdir -p "$OUT/fuzz"

echo "==> generating $CASES cases per encoding (seed $SEED)"
rm -f "$OUT"/fuzz/*.bin
(cd "$HERE" && AI_HW_ENGINEER="$HW" cargo ${RUST_TOOLCHAIN:+"+$RUST_TOOLCHAIN"} run -q --release --example rtl_fuzz -- "$OUT/fuzz" "$CASES" "$SEED")

echo "==> building the RTL runner and running the vectors in $IMAGE"
MSYS_NO_PATHCONV=1 docker run --rm -e DIFF_SHOW="${DIFF_SHOW:-2}" \
    -v "$HW:/hw:ro" -v "$HERE:/emu" "$IMAGE" sh -euc '
        mkdir -p /emu/build/rtl_runner
        verilator --cc --exe --build -j 4 --assert --public-flat-rw -Wall \
            --top-module intel_8086 --Mdir /emu/build/rtl_runner \
            /hw/design/intel_8086/src/*.sv /emu/rtl/tb_diff.cpp \
            -CFLAGS "-I/hw/design/intel_8086/tb" > /emu/build/rtl_runner/build.log 2>&1 \
            || { tail -30 /emu/build/rtl_runner/build.log; exit 1; }
        /emu/build/rtl_runner/Vintel_8086 /emu/build/fuzz/*.bin
    ' | tee "$OUT/rtl_diff.log"
