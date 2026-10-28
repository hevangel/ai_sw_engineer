#!/usr/bin/env bash
# Build a target image for the IBM PC from objects, assembly and LLVM IR.
#
#   build_image.sh <linker.ld> <out-prefix> <input>...
#
# Inputs are .o files (from llc or llvm-mc), .s files (assembled here with
# llvm-mc) and .ll files (compiled here with llc -O2). The runtime library
# objects (rt/obj) are always added; the linker drops the routines nothing
# calls. Writes <out-prefix>.{elf,bin,map,img} (see scripts/link_image.sh) and
# prints the size line. The Rust part (rustc --emit=llvm-ir for a title,
# milestone 7) feeds the .ll inputs; it does not exist yet.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
script="${1:?linker script}"; prefix="${2:?output prefix}"
shift 2
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

objs=()
n=0
for f in "$@"; do
  n=$((n + 1))
  case "$f" in
    *.o)  objs+=("$f") ;;
    *.s)  o="$tmp/$n-$(basename "$f" .s).o"
          "$backend/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$f" -o "$o"; objs+=("$o") ;;
    *.ll) o="$tmp/$n-$(basename "$f" .ll).o"
          "$backend/install/bin/llc" -mtriple=i8086 -O2 -filetype=obj "$f" -o "$o"; objs+=("$o") ;;
    *) echo "ERROR: unknown input $f" >&2; exit 2 ;;
  esac
done
exec "$here/link_image.sh" "$script" "$prefix" "${objs[@]}" "$backend"/rt/obj/*.o
