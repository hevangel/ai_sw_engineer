#!/usr/bin/env bash
# Capture GRAPHICS's cases on MartyPC's IBM 5160 (the oracle, or a rewrite's run with PROGRAM=):
# builds the capture tools (tools/build.sh, copied onto drive A by cases.py), runs
# ../../scripts/capture_dos.sh with SAVE_B=1, then reads OUT.PRN, the printed bytes PRNDUMP
# wrote, from each case's drive B image after the run (pcdisk extract) and adds a line
#   # printed: <length> bytes sha256 <hex>        (or `# printed: none` without OUT.PRN)
# after the transcript's header. The bytes stay next to the images in WORK as <case>.prn.
#
#   XT_ROMS=DIR [MARTY_TARGET_DIR=DIR] [PROGRAM=FILE NAME=GRAPHICS.COM OUT=DIR WORK=DIR] \
#     system/ibm_pc_xt_5160/graphics/scripts/capture.sh [case...]
#
# Without PROGRAM it re-captures the oracle (oracle/transcripts); never run it so unless that is
# meant.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
OUT="${OUT:-$HERE/oracle/transcripts}"
WORK="${WORK:-$HERE/build/oracle}"
export OUT WORK
mkdir -p "$WORK"
bash "$HERE/tools/build.sh" >/dev/null
SAVE_B=1 bash "$ROOT/system/ibm_pc_xt_5160/scripts/capture_dos.sh" "$HERE" "$@" > "$WORK.cases"
PCDISK="${MARTY_TARGET_DIR:-/tmp/martypc-target}/release/pcdisk"
while IFS= read -r name; do
  t="$OUT/$name.txt"
  img="$WORK/${name}_b_after.img"
  rm -f "$WORK/$name.prn"
  if [[ -f $img ]] && "$PCDISK" extract "$img" OUT.PRN "$WORK/$name.prn" 2>/dev/null; then
    line="# printed: $(wc -c < "$WORK/$name.prn") bytes sha256 $(sha256sum "$WORK/$name.prn" | cut -d' ' -f1)"
  else
    line="# printed: none"
  fi
  awk -v l="$line" '!done && /^\|/ { print l; done = 1 } { print }' "$t" > "$t.new" && mv "$t.new" "$t"
  echo "$name: ${line#\# printed: }"
done < "$WORK.cases"
rm -f "$WORK.cases"
