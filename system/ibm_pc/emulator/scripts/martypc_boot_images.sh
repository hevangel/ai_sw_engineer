#!/usr/bin/env bash
# Run the boot images of oracle/boot and the DOS programs of oracle/dos on MartyPC's IBM 5150 model
# (external/martypc, unmodified, used as a library by tools/martypc_oracle) and compare the screen with
# the transcript the RTL recorded.
#
# MartyPC's CPU and bus timing were checked against real 8088 hardware by its author, so this is a
# third, independent model of the machine (after the RTL and the Rust emulator).
#
# Run on the host with docker. Needs the firmware (bios.bin, basic.bin, workdisk.img) in
# $AI_HW_ENGINEER/system/ibm_pc/work/reference, the boot images (scripts/rtl_boot_images.sh) and
# the DOS disks (scripts/rtl_dos_programs.sh leaves them in chkdsk/build/com).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
HW="$(cd "$HW" && pwd)"
REF=/ai_hw_engineer/system/ibm_pc/work/reference
fail=0

run() { # <image relative to the emulator dir> <cycles> <transcript> [<wait text> <keys>]...
  local img=$1 cycles=$2 transcript=$3; shift 3
  local args=("$@") got want q=""
  for x in "${args[@]}"; do q+=" $(printf '%q' "$x")"; done
  got="$(MSYS_NO_PATHCONV=1 docker run --rm -v "$ROOT:/workspace" -v "$HW:/ai_hw_engineer:ro" \
      -v ai-sw-llvm-build:/build -e CARGO_TARGET_DIR=/build/martypc-target -e DRIVE_B=$REF/workdisk.img \
      -w /workspace/system/ibm_pc/emulator/tools/martypc_oracle ai-sw-engineer:latest \
      bash -c "cargo +1.99.1 run --release -q -- $REF/bios.bin $REF/basic.bin /workspace/system/ibm_pc/$img $cycles $q" 2>/dev/null)"
  want="$(grep '^|' "$HERE/$transcript" | sed 's/[[:space:]]*$//')"
  # the last 25 rows of the transcript are the final screen
  want="$(printf '%s\n' "$want" | tail -n 25)"
  if [[ "$(printf '%s\n' "$got" | sed 's/[[:space:]]*$//' | tail -n 25)" == "$want" ]]; then
    echo "ok: $transcript — MartyPC shows the RTL's screen"
  else
    echo "DIFFERENT: $transcript"; diff <(printf '%s\n' "$got" | tail -n 25) <(printf '%s\n' "$want") || true; fail=1
  fi
}

# where the images are (the toolchain profile points these at its own build)
BOOT_IMG_DIR="${BOOT_IMG_DIR:-emulator/oracle/boot/images}"
DOS_IMG_DIR="${DOS_IMG_DIR:-chkdsk/build/com}"
CASES="${CASES:-boot_hello hello_rs irq_rs hello_com hello_exe seg_exe}"
want_case() { [[ " $CASES " == *" $1 "* ]]; }
want_case boot_hello && run $BOOT_IMG_DIR/boot_hello.img 130000000 oracle/boot/boot_hello.txt
want_case hello_rs   && run $BOOT_IMG_DIR/hello_rs.img   130000000 oracle/boot/hello_rs.txt
want_case irq_rs     && run $BOOT_IMG_DIR/irq_rs.img     130000000 oracle/boot/irq_rs.txt 'ticks ok' abc
for c in "hello_com hello" "hello_exe hellox" "seg_exe segx"; do
  set -- $c
  want_case $1 || continue
  run $DOS_IMG_DIR/dos_$1.img 400000000 oracle/dos/$1.txt "Enter today's date" '10-14-81\r' 'A>' "$2"'\r'
done
exit $fail
