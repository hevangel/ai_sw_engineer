#!/usr/bin/env bash
# Run the images of build_profile.sh on MartyPC's IBM PC XT 5160 model (issue #59, execution gate)
# with the original XT BIOS, and compare the screens with expectations that do not come from
# MartyPC:
#   xt_info.img        expected/xt_info.txt (the profile's machine and the ROM file's sum)
#   hello_rs, irq_rs   the screen the ai_hw_engineer RTL recorded for the same programs on the 5150
#                      (system/ibm_pc/emulator/oracle/boot/*.txt; their output does not depend on
#                      the machine)
#   hello.com, hellox.exe, segx.exe under the original PC DOS 2.00: the lines each program printed
#                      under PC DOS 1.00 on the RTL (system/ibm_pc/emulator/oracle/dos/*.txt)
#
#   XT_ROMS=DIR system/ibm_pc_xt_5160/toolchain/scripts/run_independent.sh
#
# XT_ROMS holds xtbios.bin, xtbasic.bin and pcdos200.img (scripts/fetch_reference.py). MartyPC
# (external/martypc, unmodified, MIT) is driven by system/ibm_pc/emulator/tools/martypc_oracle with
# MARTY_MACHINE=5160 (256 KiB, CGA, two drives).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
B="$HERE/build"
R="${XT_ROMS:?set XT_ROMS (scripts/fetch_reference.py)}"
ORACLE="$ROOT/system/ibm_pc/emulator/oracle"
sha() { sha256sum "$1" | cut -d' ' -f1; }
[[ $(sha "$R/xtbios.bin") == 2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2 ]] || { echo "ERROR: xtbios.bin is not the 1982-11-08 XT BIOS" >&2; exit 1; }
[[ $(sha "$R/pcdos200.img") == 934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88 ]] || { echo "ERROR: pcdos200.img is not PC DOS 2.00 disk 1" >&2; exit 1; }

export CARGO_TARGET_DIR="${MARTY_TARGET_DIR:-/tmp/martypc-target}"
(cd "$ROOT/system/ibm_pc/emulator/tools/martypc_oracle" && cargo +1.99.1 build --quiet --release)
MARTY="$CARGO_TARGET_DIR/release/martypc_oracle"
PCDISK_DIR="$ROOT/system/ibm_pc/platform/tools/pcdisk"
(cd "$PCDISK_DIR" && cargo +1.99.1 build --quiet --release)
PCDISK="$CARGO_TARGET_DIR/release/pcdisk"
unset CARGO_TARGET_DIR

# The ROM sum the expected screen states, recomputed from the ROM file.
sum=$(python3 -I -c "import struct,sys;d=open(sys.argv[1],'rb').read();print('%#06x'%(sum(struct.unpack('<4096H',d))&0xFFFF))" "$R/xtbios.bin")
grep -q "^bios sum = $sum\$" "$HERE/expected/xt_info.txt" || { echo "ERROR: expected/xt_info.txt does not state bios sum = $sum" >&2; exit 1; }

fail=0
mkdir -p "$B/screens" "$B/disks"
marty() { MARTY_MACHINE=5160 "$MARTY" "$R/xtbios.bin" "$R/xtbasic.bin" "$@" 2>/dev/null; }
check() { # <label> compare_screen args...
  if python3 -I "$HERE/scripts/compare_screen.py" "${@:2}" | sed "s/^/    /"; then echo "ok: $1"; else echo "FAIL: $1"; fail=1; fi
}

marty "$B/xt_info.img" 300000000 > "$B/screens/xt_info.txt"
check "xt_info.img on MartyPC 5160" "$B/screens/xt_info.txt" "$HERE/expected/xt_info.txt"

for n in hello_rs irq_rs; do
  keys=(); [[ $n == irq_rs ]] && keys=('ticks ok' abc)
  marty "$B/$n.img" 300000000 "${keys[@]}" > "$B/screens/$n.txt"
  # the RTL transcript's last 25 rows are its final screen
  grep '^|' "$ORACLE/boot/$n.txt" | tail -n 25 | sed 's/[[:space:]]*$//' > "$B/screens/$n.want"
  check "$n.img on MartyPC 5160 = the RTL's screen" "$B/screens/$n.txt" "$B/screens/$n.want"
done

for c in "hello_com HELLO.COM hello.com hello" "hello_exe HELLOX.EXE hellox.exe hellox" "seg_exe SEGX.EXE segx.exe segx"; do
  set -- $c
  cp "$R/pcdos200.img" "$B/disks/dos200_$1.img"
  "$PCDISK" add "$B/disks/dos200_$1.img" "$B/$3" "$2"
  marty "$B/disks/dos200_$1.img" 600000000 'Enter new date' '\r' 'Enter new time' '\r' 'A>' "$4"'\r' > "$B/screens/dos200_$1.txt"
  check "$3 under PC DOS 2.00 on MartyPC 5160 = the program's lines on the RTL" \
    "$B/screens/dos200_$1.txt" "$ORACLE/dos/$1.txt" "$4"
done
(cd "$B/disks" && sha256sum dos200_*.img > ../disks.sha256)
exit $fail
