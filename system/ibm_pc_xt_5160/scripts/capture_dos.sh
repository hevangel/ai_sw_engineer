#!/usr/bin/env bash
# Capture a DOS title's oracle (or a rewrite's run) on MartyPC's IBM 5160: one boot per case of
# <title>/scripts/cases.py. For each case its input files are added to a copy of the original
# PC DOS 2.00 disk 1 with pcdisk, DOS boots (date and time answered with Enter), the command is
# typed at A>, and the 25 screen rows go to <out>/<case>.txt.
#
#   XT_ROMS=DIR system/ibm_pc_xt_5160/scripts/capture_dos.sh <title dir> [case...]
#
# cases.py DIR writes the case files to DIR and prints one tab-separated line per case:
# name, comma-separated files (- for none), command, then optional pairs of (text to wait for,
# keys to type) for prompts after the command. A file item may instead be a step for a drive B
# disk (a blank 360 KiB disk made for the case): b:mkdir:PATH, b:label:LABEL, b:add:PATH=FILE.
# PROGRAM=<file> with NAME=<8.3 name> puts a rewrite on the disk in place of the original; OUT and WORK choose where transcripts and disks go
# (default <title>/oracle/transcripts, <title>/build/oracle).
set -euo pipefail
TITLE="$(cd "${1:?title dir}" && pwd)"; shift
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
R="${XT_ROMS:?set XT_ROMS (toolchain/scripts/fetch_reference.py)}"
OUT="${OUT:-$TITLE/oracle/transcripts}"
WORK="${WORK:-$TITLE/build/oracle}"
sha() { sha256sum "$1" | cut -d' ' -f1; }
[[ $(sha "$R/pcdos200.img") == 934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88 ]] || { echo "ERROR: pcdos200.img is not PC DOS 2.00 disk 1" >&2; exit 1; }
[[ $(sha "$R/xtbios.bin") == 2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2 ]] || { echo "ERROR: xtbios.bin is not the 1982-11-08 XT BIOS" >&2; exit 1; }
export CARGO_TARGET_DIR="${MARTY_TARGET_DIR:-/tmp/martypc-target}"
(cd "$ROOT/system/ibm_pc/emulator/tools/martypc_oracle" && cargo +1.99.1 build --quiet --release)
(cd "$ROOT/system/ibm_pc/platform/tools/pcdisk" && cargo +1.99.1 build --quiet --release)
MARTY="$CARGO_TARGET_DIR/release/martypc_oracle"; PCDISK="$CARGO_TARGET_DIR/release/pcdisk"
unset CARGO_TARGET_DIR
mkdir -p "$OUT" "$WORK/in"
python3 -I "$TITLE/scripts/cases.py" "$WORK/in" > "$WORK/cases.tsv"
want=" $* "
while IFS= read -r line; do
  IFS=$'\t' read -ra f <<< "$line"
  name=${f[0]} files=${f[1]} cmd=${f[2]}
  more=("${f[@]:3}")
  [[ $# -gt 0 && $want != *" $name "* ]] && continue
  disk="$WORK/$name.img"
  cp "$R/pcdos200.img" "$disk"
  [[ -n ${PROGRAM:-} ]] && "$PCDISK" add "$disk" "$PROGRAM" "${NAME:?NAME=<8.3 name of the program>}"
  diskb=""
  if [[ $files != - ]]; then
    IFS=, read -ra fl <<< "$files"
    for x in "${fl[@]}"; do
      case $x in
        b:*)
          if [[ -z $diskb ]]; then diskb="$WORK/${name}_b.img"; "$PCDISK" blank "$diskb" 360; fi
          step=${x#b:}
          case $step in
            mkdir:*) "$PCDISK" mkdir "$diskb" "${step#mkdir:}" ;;
            label:*) "$PCDISK" label "$diskb" "${step#label:}" ;;
            add:*) spec=${step#add:}; "$PCDISK" add "$diskb" "$WORK/in/${spec#*=}" "${spec%%=*}" ;;
            *) echo "ERROR: unknown drive B step $x" >&2; exit 1 ;;
          esac ;;
        *) "$PCDISK" add "$disk" "$WORK/in/$x" "$x" ;;
      esac
    done
  fi
  rows="$(DRIVE_B=${diskb:-} MARTY_MACHINE=5160 MARTY_WRITABLE=1 MARTY_SETTLE="${MARTY_SETTLE:-80000000}" \
      MARTY_KEY_DELAY="${MARTY_KEY_DELAY:-2000000}" \
      "$MARTY" "$R/xtbios.bin" "$R/xtbasic.bin" "$disk" 700000000 \
      'Enter new date' '\r' 'Enter new time' '\r' 'A>' "$cmd"'\r' "${more[@]}" 2>/dev/null)"
  {
    echo "# oracle: $cmd"
    echo "# machine: MartyPC 05c0d088 IBM 5160, 256 KiB, XT BIOS 1982-11-08, PC DOS 2.00 disk 1 + $files"
    if [[ -n ${PROGRAM:-} ]]; then
      echo "# program: $NAME = $(basename "$PROGRAM") sha256 $(sha "$PROGRAM")"
    else
      echo "# program: the original on the disk"
    fi
    printf '%s\n' "$rows"
  } > "$OUT/$name.txt"
  echo "$name"
done < "$WORK/cases.tsv"
