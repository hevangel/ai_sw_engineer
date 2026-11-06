#!/usr/bin/env bash
# Run the images of build_profile.sh on 86Box's IBM PS/2 Model 30 (issue #60, execution gate) with
# the original BIOS revision 0, and compare the screens with expectations that do not come from 86Box:
#   ps2_info.img       expected/ps2_info.txt (the profile's machine, the ROM file's model byte,
#                      date and sum)
#   hello_rs, irq_rs   the screen the ai_hw_engineer RTL recorded for the same programs on the 5150
#                      (system/ibm_pc/emulator/oracle/boot/*.txt; their output does not depend on
#                      the machine)
#   hello.com, hellox.exe, segx.exe under the original PC DOS 3.30 (the 720 KiB boot disk of
#                      fetch_reference.py): the lines each program printed under PC DOS 1.00 on the
#                      RTL, except "drive A of N": N is what DOS function 0Eh returns, the number of
#                      drives under PC DOS 1.x/2.x but the number of logical drive letters, at least
#                      5 (LASTDRIVE), from DOS 3.0 on (Microsoft, MS-DOS Programmer's Reference,
#                      INT 21h function 0Eh), so 5 here
#
#   PS2_ROMS=DIR BOX86=<86Box> system/ibm_ps2_model30/toolchain/scripts/run_independent.sh
#
# BOX86 is 86Box built with -D AUTOMATION=ON (report/report.md has the commit and the build).
# POST stops at "161-Set Date and Time" and "1790-Fixed Disk Error" (no clock set, no fixed disk:
# both true of this machine) and waits for F1, which the script presses.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
B="$HERE/build"
R="${PS2_ROMS:?set PS2_ROMS (scripts/fetch_reference.py)}"
BOX="${BOX86:?set BOX86 to an 86Box built with -D AUTOMATION=ON}"
ORACLE="$ROOT/system/ibm_pc/emulator/oracle"
CMP="$ROOT/system/ibm_pc_xt_5160/toolchain/scripts/compare_screen.py"
sha() { sha256sum "$1" | cut -d' ' -f1; }
[[ $(sha "$R/ps2bios.bin") == e3de87dd253468fc3f739f6d07f4444f58bc1b34e1ebfc088d125d5db048c6f7 ]] || { echo "ERROR: ps2bios.bin is not BIOS revision 0" >&2; exit 1; }
[[ $(sha "$R/pcdos330_720k.img") == ec8965c66541e2b981fcb65c57f9d9322d0c94c5bf48b169eee207e9c7fc3404 ]] || { echo "ERROR: pcdos330_720k.img is not the pinned PC DOS 3.30 disk" >&2; exit 1; }
export CARGO_TARGET_DIR="${PCDISK_TARGET_DIR:-/tmp/pcdisk-target}"
(cd "$ROOT/system/ibm_pc/platform/tools/pcdisk" && cargo +1.99.1 build --quiet --release)
PCDISK="$CARGO_TARGET_DIR/release/pcdisk"
unset CARGO_TARGET_DIR

sum=$(python3 -I -c "import struct,sys;d=open(sys.argv[1],'rb').read();print('%#06x'%(sum(struct.unpack('<32768H',d))&0xFFFF))" "$R/ps2bios.bin")
grep -q "^bios sum = $sum\$" "$HERE/expected/ps2_info.txt" || { echo "ERROR: expected/ps2_info.txt does not state bios sum = $sum" >&2; exit 1; }

fail=0
mkdir -p "$B/screens" "$B/disks"
run() { python3 -I "$HERE/scripts/ps2_86box.py" "$BOX" "$R" "$@" 2>/dev/null; }
check() {
  if python3 -I "$CMP" "${@:2}" | sed "s/^/    /"; then echo "ok: $1"; else echo "FAIL: $1"; fail=1; fi
}

run "$B/ps2_info.img" 120 RESUME '{F1}' done '' > "$B/screens/ps2_info.txt"
check "ps2_info.img on 86Box PS/2 Model 30" "$B/screens/ps2_info.txt" "$HERE/expected/ps2_info.txt"

for n in hello_rs irq_rs; do
  keys=(RESUME '{F1}' done ''); [[ $n == irq_rs ]] && keys=(RESUME '{F1}' 'ticks ok' abc done '')
  run "$B/$n.img" 120 "${keys[@]}" > "$B/screens/$n.txt"
  grep '^|' "$ORACLE/boot/$n.txt" | tail -n 25 | sed 's/[[:space:]]*$//' > "$B/screens/$n.want"
  check "$n.img on 86Box PS/2 Model 30 = the RTL's screen" "$B/screens/$n.txt" "$B/screens/$n.want"
done

for c in "hello_com HELLO.COM hello.com hello" "hello_exe HELLOX.EXE hellox.exe hellox" "seg_exe SEGX.EXE segx.exe segx"; do
  set -- $c
  cp "$R/pcdos330_720k.img" "$B/disks/dos330_$1.img"
  "$PCDISK" add "$B/disks/dos330_$1.img" "$B/$3" "$2"
  run "$B/disks/dos330_$1.img" 180 RESUME '{F1}' 'Enter new date' '\r' 'Enter new time' '\r' 'A>' "$4"'\r' > "$B/screens/dos330_$1.txt"
  sed 's/^|drive A of 2$/|drive A of 5/' "$ORACLE/dos/$1.txt" > "$B/screens/dos330_$1.want"
  check "$3 under PC DOS 3.30 on 86Box PS/2 Model 30 = the program's lines on the RTL" \
    "$B/screens/dos330_$1.txt" "$B/screens/dos330_$1.want" "$4"
done
exit $fail
