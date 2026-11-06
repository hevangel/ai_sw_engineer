#!/usr/bin/env bash
# Capture the oracle: the original SORT.EXE of PC DOS 2.00 on MartyPC's IBM 5160 (the XT profile's
# independent emulator), one boot per case of scripts/cases.py. For each case the input file is
# added to a copy of the original PC DOS 2.00 disk 1 with pcdisk, DOS boots (date and time
# prompts answered with Enter), the command is typed at A> (drive A writable: pipes need it), and the 25 screen rows are written to
# oracle/transcripts/<case>.txt with a header naming the command and the disk's SHA-256.
#
#   XT_ROMS=DIR system/ibm_pc_xt_5160/sort/scripts/capture_oracle.sh [case...]
#
# XT_ROMS: xtbios.bin, xtbasic.bin, pcdos200.img (../toolchain/scripts/fetch_reference.py).
# Also used with PROGRAM=<file.exe> OUT=<dir> to run the Rust rewrite in place of the original
# (copied over SORT.EXE on the disk) with the same cases.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
R="${XT_ROMS:?set XT_ROMS (toolchain/scripts/fetch_reference.py)}"
OUT="${OUT:-$HERE/oracle/transcripts}"
WORK="${WORK:-$HERE/build/oracle}"
sha() { sha256sum "$1" | cut -d' ' -f1; }
[[ $(sha "$R/pcdos200.img") == 934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88 ]] || { echo "ERROR: pcdos200.img is not PC DOS 2.00 disk 1" >&2; exit 1; }
[[ $(sha "$R/xtbios.bin") == 2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2 ]] || { echo "ERROR: xtbios.bin is not the 1982-11-08 XT BIOS" >&2; exit 1; }

export CARGO_TARGET_DIR="${MARTY_TARGET_DIR:-/tmp/martypc-target}"
(cd "$ROOT/system/ibm_pc/emulator/tools/martypc_oracle" && cargo +1.99.1 build --quiet --release)
(cd "$ROOT/system/ibm_pc/platform/tools/pcdisk" && cargo +1.99.1 build --quiet --release)
MARTY="$CARGO_TARGET_DIR/release/martypc_oracle"
PCDISK="$CARGO_TARGET_DIR/release/pcdisk"
unset CARGO_TARGET_DIR

mkdir -p "$OUT" "$WORK/in"
python3 -I "$HERE/scripts/cases.py" "$WORK/in" > "$WORK/cases.txt"
want=("$@")
while read -r name file _ _ cmd; do
  if [[ ${#want[@]} -gt 0 && " ${want[*]} " != *" $name "* ]]; then continue; fi
  disk="$WORK/$name.img"
  cp "$R/pcdos200.img" "$disk"
  if [[ -n ${PROGRAM:-} ]]; then "$PCDISK" add "$disk" "$PROGRAM" SORT.EXE; fi
  "$PCDISK" add "$disk" "$WORK/in/$file" "$file"
  # the typed command line: lower case as cases.py has it
  rows="$(MARTY_MACHINE=5160 MARTY_WRITABLE=1 "$MARTY" "$R/xtbios.bin" "$R/xtbasic.bin" "$disk" 700000000 \
      'Enter new date' '\r' 'Enter new time' '\r' 'A>' "$cmd"'\r' 2>/dev/null)"
  {
    echo "# SORT oracle: $cmd"
    echo "# machine: MartyPC 05c0d088 IBM 5160, 256 KiB, XT BIOS 1982-11-08, PC DOS 2.00 disk 1 + $file"
    echo "# program: ${PROGRAM:+$(basename "$PROGRAM") (sha256 $(sha "$PROGRAM"))}${PROGRAM:-original SORT.EXE (sha256 755a3c1c...)}"
    echo "# input sha256 $(sha "$WORK/in/$file")"
    printf '%s\n' "$rows"
  } > "$OUT/$name.txt"
  echo "$name: $(printf '%s\n' "$rows" | grep -c . ) rows"
done < <(sed 's/  */ /g' "$WORK/cases.txt" | awk '{name=$1; file=$2; $1=$2=$3=$4=""; sub(/^ +/,""); print name, file, "x", "x", $0}')
