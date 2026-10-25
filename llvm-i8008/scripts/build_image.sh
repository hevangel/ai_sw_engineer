#!/usr/bin/env bash
# Link an i8008 image (spec/codegen.md sec.6).
#
#   build_image.sh <layout.ld> <out.bin> <input>...
#
# Inputs are .o files (from build_rust.sh) or .s files (assembled here with
# llvm-mc). The rt/ helpers are always added; the linker drops the ones
# nothing calls. Writes <out.bin> and <out.map>; prints the size line.
set -euo pipefail
layout="${1:?layout}"
out="${2:?out.bin}"
shift 2
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
mc="$backend/install/bin/llvm-mc"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

objs=()
n=0
for f in "$@" "$backend"/rt/*.s; do
  case "$f" in
    *.s) n=$((n + 1)); o="$tmp/$n-$(basename "$f" .s).o"
         "$mc" -triple i8008 -filetype=obj "$f" -o "$o"; objs+=("$o") ;;
    *.o) objs+=("$f") ;;
    *) echo "ERROR: unknown input $f" >&2; exit 2 ;;
  esac
done
python3 "$backend/asm/i8008_link.py" "${objs[@]}" -T "$layout" -o "$out" -m "${out%.bin}.map"
