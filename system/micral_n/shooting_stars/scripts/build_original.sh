#!/usr/bin/env bash
# Build the oracle: the original Shooting Stars with the Micral N teletype.
#   build_original.sh [out.bin]   (default build/original.bin)
# original/shooter.asm (fetch.sh) up to its "cpu 8008new" line, plus
# oracle/micral_io.asm, assembled by platform/scripts/as8008.py.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$here/build/original.bin}"
mkdir -p "$(dirname "$out")" "$here/build"
src="$here/original/shooter.asm"
[[ -f $src ]] || "$here/original/fetch.sh"
{ sed '/cpu 8008new/,$d' "$src"; cat "$here/oracle/micral_io.asm"; } > "$here/build/original.asm"
python3 "$here/../platform/scripts/as8008.py" "$here/build/original.asm" "$out" \
  --origin 0 --listing "$here/build/original.lst"
