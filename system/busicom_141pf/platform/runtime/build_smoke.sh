#!/usr/bin/env bash
# Build the busicom_141pf HAL smoke image: crt0 + hal + smoke, linked by the
# i4004 packer into a 1280-byte ROM .bin. Runs inside the ai-sw-engineer
# container (llvm-mc on the ai-sw-llvm-build volume).
#
#   docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
#     ai-sw-engineer:latest bash system/busicom_141pf/platform/runtime/build_smoke.sh <out.bin>
set -euo pipefail
OUT="${1:?output .bin path}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
RTDIR="$ROOT/system/busicom_141pf/platform/runtime"
LLVMRT="$ROOT/llvm-i4004/rt"
B="/llvm-build/llvm-obj/bin"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
W="$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/smoke_build.XXXXXX")"

[[ -x "$B/llvm-mc" ]] || { echo "ERROR: llvm-mc missing at $B" >&2; exit 1; }

objs=()
for s in "$LLVMRT/crt0.s" "$RTDIR/hal.s" "$RTDIR/smoke.s"; do
    o="$W/$(basename "$s" .s).o"
    "$B/llvm-mc" -triple i4004 --filetype=obj "$s" -o "$o"
    objs+=("$o")
done
python3 "$PACK" "${objs[@]}" -o "$OUT" --pad-pages --pad 1280 \
    -m "${OUT%.bin}.map"
echo "built $OUT ($(wc -c < "$OUT") bytes); map ${OUT%.bin}.map" >&2
