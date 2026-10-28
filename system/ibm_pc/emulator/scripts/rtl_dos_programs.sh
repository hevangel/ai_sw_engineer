#!/usr/bin/env bash
# Record oracle/dos/hello_com.txt and hello_exe.txt: the Rust hello program as a .COM and as
# a tiny-model .EXE, run under the unchanged PC DOS 1.00 on the hardware project's RTL.
#
# Links the programs with i8086-ld, puts them on a copy of the DOS disk with pcdisk, and runs
# chkdsk/scripts/rtl_oracle.sh (about 3 minutes each). Needs cargo on the host.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
HW="$(cd "$HW" && pwd)"
PLAT="$ROOT/system/ibm_pc/platform"
WORK="$ROOT/system/ibm_pc/chkdsk/build/com"
mkdir -p "$WORK"
(cd "$ROOT/llvm-i8086/asm" && cargo +"${RUST_TOOLCHAIN:-1.99.1}" build --quiet --release)
(cd "$PLAT/tools/pcdisk" && cargo +"${RUST_TOOLCHAIN:-1.99.1}" build --quiet --release)
LD="$ROOT/llvm-i8086/asm/target/release/i8086-ld"
PCDISK="$PLAT/tools/pcdisk/target/release/pcdisk"
"$LD" -T "$PLAT/link_com.ld" --com "$WORK/hello.com" --exe "$WORK/hellox.exe" \
    "$PLAT/runtime/crt0_com.o" "$PLAT/runtime/dos.o" \
    "$ROOT/llvm-i8086/tests/smoke/hello_com/hello_com.o" "$ROOT/llvm-i8086/rt/librt.a"
rec() { # <name> <file on the disk> <command>
    cp "$HW/system/ibm_pc/work/reference/pcdos100.img" "$WORK/dos_$1.img"
    "$PCDISK" add "$WORK/dos_$1.img" "$2" "$3"
    DOS_IMG="$WORK/dos_$1.img" OUT_DIR="$HERE/oracle/dos" \
        "$ROOT/system/ibm_pc/chkdsk/scripts/rtl_oracle.sh" "$1" "$ROOT/system/ibm_pc/chkdsk/build/blank.img" "$4"
}
[[ -f "$ROOT/system/ibm_pc/chkdsk/build/blank.img" ]] || "$PCDISK" blank "$ROOT/system/ibm_pc/chkdsk/build/blank.img"
[[ -n "${ONLY:-}" && "$ONLY" != hello_com ]] || rec hello_com "$WORK/hello.com" HELLO.COM hello
[[ -n "${ONLY:-}" && "$ONLY" != hello_exe ]] || rec hello_exe "$WORK/hellox.exe" HELLOX.EXE hellox
# the two-segment .EXE (llvm-i8086/tests/smoke_ir/seg_exe.ll): a far function and its data in
# a second segment, found by the segment relocations of the MZ header
"$LD" -T "$PLAT/link_exe_seg.ld" --exe "$WORK/segx.exe" "$ROOT/llvm-i8086/tests/smoke_ir/seg_exe.o"
[[ -n "${ONLY:-}" && "$ONLY" != seg_exe ]] || rec seg_exe "$WORK/segx.exe" SEGX.EXE segx
