#!/usr/bin/env bash
# Build a target ROM image for a software title with the llvm-i4004 backend.
#
#   Usage: build_image.sh <title_dir> <out_dir> <linker_script>
#   Produces <out_dir>/<software>.{s,o,bin,map} and prints the image size.
#
# Pipeline (abi.md / the brief):
#   firmware IR  (build_target_ir.sh: rustc avr-none -> retarget_ir.py)
#     -> llc -mtriple=i4004 -filetype=obj            (one firmware .o)
#     -> llvm-mc  crt0.s + libi4004rt/*.s + hal.s    (runtime .o's)
#     -> i4004_pack.py <objs> --linker-script <ld>   (link + page-pack -> .bin)
#
# Runs inside the ai-sw-engineer container. Tools come from
# llvm-i4004/install/bin (built by scripts/build_llvm.sh) or, as a fallback,
# the shared /llvm-build volume.
#
# Board/budget: the default linker script here is the xl board's link_xl.ld
# (16 x 4001 ROM = 4096 B); pass a different <linker_script> for the stock
# board. BOARD env (stock|xl, default inferred from the linker-script name)
# selects --max.
set -euo pipefail

title_dir="${1:?title_dir}"
out_dir="${2:?out_dir}"
link="${3:?linker_script}"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
rt="$root/rt"
bin="$root/install/bin"
if [[ ! -x "$bin/llc" ]]; then
  if [[ -x /llvm-build/llvm-obj/bin/llc ]]; then bin="/llvm-build/llvm-obj/bin"; fi
fi
[[ -x "$bin/llc" ]] || { echo "ERROR: llc not found in $root/install/bin or /llvm-build (run build_llvm.sh)" >&2; exit 2; }

software="$(basename "$title_dir")"
mkdir -p "$out_dir"
status(){ printf '==> %s\n' "$*" >&2; }

# ROM budget from the linker-script name (xl = 4096, else stock 1280).
case "${BOARD:-}" in
  xl)    maxrom=4096 ;;
  stock) maxrom=1280 ;;
  *) if [[ "$link" == *xl* ]]; then maxrom=4096; else maxrom=1280; fi ;;
esac

# 1. firmware IR -> object. Rebuild the IR if it is missing.
ir="$title_dir/build/firmware_i4004.ll"
if [[ ! -f "$ir" ]]; then
  status "firmware IR missing; building it (build_target_ir.sh)"
  "$title_dir/scripts/build_target_ir.sh" >&2
fi
# Flat image (default, issue #18): JMS/BBL calls, bank-0 RAM without DCL,
# page-local branches relaxed by the packer only where they cross a page, and
# unreferenced runtime helpers dropped (--gc-sections). LEGACY_BANKED=1 keeps
# the frozen soft-call / DCL-banked / bank-latched flow (BANKS=N etc.).
llc_args=( -mtriple=i4004 -function-sections -filetype=obj )
crt0=crt0.s
if [[ "${LEGACY_BANKED:-0}" == "1" ]]; then
  llc_args+=( -i4004-softcall -i4004-banked-ram -i4004-relax-all-branches )
  crt0=crt0_banked.s
fi
status "llc: firmware IR -> $out_dir/$software.o"
timeout 180 "$bin/llc" "${llc_args[@]}" "$ir" -o "$out_dir/$software.o"

# 2. assemble crt0 + runtime + HAL. crt0 MUST be the first object so .reset
#    lands at ROM 0 (the packer places sections in object order).
hal="$root/../system/busicom_141pf/platform/runtime/hal.s"
rtsrcs=(arith.s arith16.s bcd.s logic.s logic16.s shift.s shift16.s mem.s stack.s regops.s)
objs=("$out_dir/crt0.o")
timeout 60 "$bin/llvm-mc" -triple i4004 --filetype=obj "$rt/$crt0" -o "$out_dir/crt0.o"
for s in "${rtsrcs[@]}"; do
  b="$(basename "$s" .s)"
  timeout 60 "$bin/llvm-mc" -triple i4004 --filetype=obj "$rt/$s" -o "$out_dir/$b.o"
  objs+=("$out_dir/$b.o")
done
timeout 60 "$bin/llvm-mc" -triple i4004 --filetype=obj "$hal" -o "$out_dir/hal.o"
objs+=("$out_dir/hal.o")
# The title's own hand-written routines (e.g. the Busicom BCD primitives).
for s in "$title_dir"/asm/*.s; do
  [[ -f $s ]] || continue
  b="title_$(basename "$s" .s)"
  timeout 60 "$bin/llvm-mc" -triple i4004 --filetype=obj "$s" -o "$out_dir/$b.o"
  objs+=("$out_dir/$b.o")
done
if [[ -n "${EXTRA_OBJS:-}" ]]; then
  # extra hand-written objects (e.g. the firmware's arithmetic primitives)
  for o in $EXTRA_OBJS; do objs+=("$o"); done
fi

# firmware object LAST (crt0 first for .reset@0; the packer resolves cross-
# object relocations regardless of order).
objs+=("$out_dir/$software.o")

# 3. link + page-pack into a ROM image.
status "i4004_pack: link $((${#objs[@]})) objects -> $out_dir/$software.bin (max $maxrom)"
pack_args=( --linker-script "$link" --max "$maxrom" --pad "$maxrom" )
if [[ "${LEGACY_BANKED:-0}" == "1" ]]; then
  # --softcall synthesizes the software-call return dispatch (__soft_ret,
  # __srtab, per-page jin-p0 stubs, __push_off/__push_pg).
  pack_args+=( --softcall )
  if [[ -n "${BANKS:-}" ]]; then
      pack_args+=( --banks "$BANKS" )
      [[ -n "${COMMON_BYTES:-}" ]] && pack_args+=( --common-bytes "$COMMON_BYTES" )
      [[ -n "${BANK_BYTES:-}" ]] && pack_args+=( --bank-bytes "$BANK_BYTES" )
  fi
else
  pack_args+=( --gc-sections )
fi
timeout 180 python3 "$root/asm/i4004_pack.py" "${objs[@]}" \
    -o "$out_dir/$software.bin" -m "$out_dir/$software.map" \
    "${pack_args[@]}"

size=$(wc -c < "$out_dir/$software.bin" | tr -d ' ')
status "image: $out_dir/$software.bin = $size bytes (budget $maxrom)"
echo "$size"
