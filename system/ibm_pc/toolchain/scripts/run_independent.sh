#!/usr/bin/env bash
# Run the images built by build_profile.sh on independent models of the 5150 and compare with what
# the RTL showed (issue #58, execution gate). Run on the host with docker; after build_profile.sh.
#
# 1. Each image must be byte for byte the one the RTL ran: the boot floppies by the SHA-256 in
#    oracle/boot/<name>.txt, the DOS disks (PC DOS 1.00 + the program, made here with pcdisk) by the
#    SHA-256 in oracle/dos/<name>.txt. So the RTL transcripts are the recorded behaviour of these very files.
# 2. MartyPC (external/martypc, validated against real 8088 hardware) boots them and its screen must be
#    the RTL's, row for row (emulator/scripts/martypc_boot_images.sh).
# The Rust emulator runs the same images in `cargo test` (boot_oracle, i8086_dos), see run_tests.sh.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
EMU="$ROOT/system/ibm_pc/emulator"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
HW="$(cd "$HW" && pwd)"
REF="$HW/system/ibm_pc/work/reference"
B="$HERE/build"
PCDISK="$ROOT/system/ibm_pc/platform/tools/pcdisk/target/release/pcdisk"
[[ -x $PCDISK ]] || (cd "$ROOT/system/ibm_pc/platform/tools/pcdisk" && cargo +1.99.1 build --quiet --release)
sha() { sha256sum "$1" | cut -d' ' -f1; }
want() { grep -m1 "^# $2" "$EMU/oracle/$1.txt" | awk '{print $NF}'; }

echo "==> the DOS disks"
mkdir -p "$B/disks"
[[ $(sha "$REF/pcdos100.img") == 29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2 ]] \
  || { echo "ERROR: pcdos100.img is not the PC DOS 1.00 disk" >&2; exit 1; }
for c in "hello_com HELLO.COM hello.com" "hello_exe HELLOX.EXE hellox.exe" "seg_exe SEGX.EXE segx.exe"; do
  set -- $c
  cp "$REF/pcdos100.img" "$B/disks/dos_$1.img"
  "$PCDISK" add "$B/disks/dos_$1.img" "$B/$3" "$2"
  [[ $(sha "$B/disks/dos_$1.img") == $(want dos/$1 'dos image sha256') ]] \
    || { echo "ERROR: dos_$1.img is not the disk the RTL ran" >&2; exit 1; }
  echo "    dos_$1.img == the RTL's disk"
done
for n in hello_rs irq_rs; do
  [[ $(sha "$B/$n.img") == $(want boot/$n 'image sha256') ]] \
    || { echo "ERROR: $n.img is not the floppy the RTL booted" >&2; exit 1; }
  echo "    $n.img == the RTL's floppy"
done

echo "==> MartyPC"
BOOT_IMG_DIR=toolchain/build DOS_IMG_DIR=toolchain/build/disks CASES="hello_rs irq_rs hello_com hello_exe seg_exe" \
  bash "$EMU/scripts/martypc_boot_images.sh"
