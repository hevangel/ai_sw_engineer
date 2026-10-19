#!/usr/bin/env bash
# Build a busicom_141pf xl-board memory-test image: crt0 + <test>.s, linked by
# the i4004 packer into a 4096-byte (16 x 4001) ROM .bin. Runs inside the
# ai-sw-engineer container (llvm-mc on the ai-sw-llvm-build volume).
#
#   docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
#     ai-sw-engineer:latest \
#     bash system/busicom_141pf/platform/runtime/tests/build_memtest_xl.sh <out.bin> [test_src.s]
#
# test_src.s defaults to memtest_xl.s (bank-0 sweep, runs on chips + RTL). Pass
# memtest_xl_allbanks.s for the full 4-bank sweep (RTL only).
set -euo pipefail
OUT="${1:?output .bin path}"
SRC_NAME="${2:-memtest_xl.s}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd)"
TDIR="$ROOT/system/busicom_141pf/platform/runtime/tests"
LLVMRT="$ROOT/llvm-i4004/rt"
LD="$ROOT/system/busicom_141pf/platform/link_xl.ld"
B="/llvm-build/llvm-obj/bin"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
W="$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/memtest_build.XXXXXX")"

[[ -x "$B/llvm-mc" ]] || { echo "ERROR: llvm-mc missing at $B" >&2; exit 1; }

objs=()
for s in "$LLVMRT/crt0.s" "$TDIR/$SRC_NAME"; do
    o="$W/$(basename "$s" .s).o"
    "$B/llvm-mc" -triple i4004 --filetype=obj "$s" -o "$o"
    objs+=("$o")
done
# xl board: 16 x 4001 = 4096-byte image; use link_xl.ld regions, pad all pages.
python3 "$PACK" "${objs[@]}" -o "$OUT" --max 4096 --pad 4096 --pad-pages \
    --linker-script "$LD" -m "${OUT%.bin}.map"
echo "built $OUT ($(wc -c < "$OUT") bytes) from $SRC_NAME; map ${OUT%.bin}.map" >&2
