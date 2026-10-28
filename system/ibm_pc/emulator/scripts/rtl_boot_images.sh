#!/usr/bin/env bash
# Record the RTL's boot of the llvm-i8086 floppy images in oracle/boot/*.txt.
#
# 1. links the images from the committed objects with i8086-ld (in the
#    ai-sw-engineer image);
# 2. boots each on the hardware project's RTL system build (in the
#    ai-hw-engineer image) with scripts/rtl_boot_image.py.
#
# Usage: scripts/rtl_boot_images.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
HW="$(cd "$HW" && pwd)"
SIM="$HW/system/ibm_pc/work/obj/Vibm_pc"
[[ -f $SIM ]] || { echo "ERROR: RTL system build not found: $SIM" >&2; exit 1; }
IMG="$HERE/oracle/boot/images"
mkdir -p "$IMG"

echo "==> linking the floppy images"
MSYS_NO_PATHCONV=1 docker run --rm -v "$ROOT:/workspace" -w /workspace \
  -e CARGO_TARGET_DIR=/tmp/i8086-ld-target ai-sw-engineer:latest bash -c '
    set -e
    L=system/ibm_pc/platform/link.ld
    O=system/ibm_pc/emulator/oracle/boot/images
    llvm-i8086/scripts/link_image.sh $L $O/boot_hello llvm-i8086/tests/boot/hello.o
    llvm-i8086/scripts/link_image.sh $L $O/hello_rs \
      system/ibm_pc/platform/runtime/crt0.o system/ibm_pc/platform/runtime/hal.o \
      llvm-i8086/tests/smoke/hello_rs/hello_rs.o llvm-i8086/rt/obj/*.o
    llvm-i8086/scripts/link_image.sh $L $O/irq_rs       system/ibm_pc/platform/runtime/crt0.o system/ibm_pc/platform/runtime/hal.o       llvm-i8086/tests/smoke/irq_rs/irq_rs.o llvm-i8086/rt/obj/*.o'

echo "==> booting them on the RTL"
rec() { # <name> <marker row(s), joined with |> [<keys typed between the markers, joined with |>]
  MSYS_NO_PATHCONV=1 docker run --rm -v "$HW:/hw:ro" -v "$HERE:/emu" "${DOCKER_IMAGE:-ai-hw-engineer:latest}" \
    sh -euc "cd /tmp && python3 /emu/scripts/rtl_boot_image.py /hw/system/ibm_pc/work/obj/Vibm_pc /hw/system/ibm_pc/work/reference /emu/oracle/boot/images/$1.img '$2' /emu/oracle/boot/$1.txt ${3:-}"
}
rec boot_hello 'llvm-i8086: stage 2 ok'
rec hello_rs 'done'
rec irq_rs 'ticks ok|done' abc
echo "==> wrote oracle/boot/*.txt (the .img files are build products; see .gitignore)"
