#!/usr/bin/env bash
# Link objects into an image with i8086-ld (llvm-i8086/asm), building the
# linker first if needed.
#
#   link_image.sh <linker.ld> <out-prefix> <object.o>... [-- extra i8086-ld options]
#
# Writes <out-prefix>.elf, .bin, .map, and .img (a bootable floppy, when the
# script has a boot region at 0x7C00). Set I8086_LD to use a built linker, and
# CARGO_TARGET_DIR to build outside the source tree.
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
script="${1:?linker script}"; prefix="${2:?output prefix}"
shift 2
objs=(); extra=()
while [[ $# -gt 0 ]]; do
  if [[ $1 == -- ]]; then shift; extra=("$@"); break; fi
  objs+=("$1"); shift
done
[[ ${#objs[@]} -gt 0 ]] || { echo "ERROR: no objects" >&2; exit 2; }

LD="${I8086_LD:-}"
if [[ -z $LD ]]; then
  TARGET="${CARGO_TARGET_DIR:-$BACKEND_DIR/asm/target}"
  CARGO_TARGET_DIR="$TARGET" cargo build --quiet --release --manifest-path "$BACKEND_DIR/asm/Cargo.toml"
  LD="$TARGET/release/i8086-ld"
  [[ -x $LD ]] || LD="$LD.exe"
fi
"$LD" -T "$script" -o "$prefix.elf" --bin "$prefix.bin" -Map "$prefix.map" \
  --floppy "$prefix.img" --sign "${extra[@]}" "${objs[@]}"
