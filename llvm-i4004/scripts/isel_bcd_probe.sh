#!/usr/bin/env bash
# Milestone 3b probe: extract the BCD digit-add IR, adapt it to the i4004
# datalayout/triple (abi.md §1), and run our llc. This is the yardstick
# function for instruction selection -- a BCD digit add from
# system/busicom_141pf/firmware/src/bcd.rs (`add_mag`).
#
# It does NOT modify the firmware crate (another agent owns it). It transcribes
# the single nibble-add loop into a standalone no_std function so it has a
# stable, selectable signature, emits IR with the pinned nightly rustc (LLVM
# 23.1.3, the same LLVM our backend is built from), swaps in the i4004 data
# layout + triple, and feeds it to llc.
#
# Run inside the ai-sw-engineer container:
#   docker run --rm -v B:\ai_sw_engineer_dis:/workspace -w /workspace \
#     ai-sw-engineer:latest bash -lc 'llvm-i4004/scripts/isel_bcd_probe.sh'
#
# CURRENT RESULT (milestone 3b, codegen backbone NOT yet built): llc reports
#   "unable to get target for 'i4004'"
# because the installed toolchain registers only the MC layer + disassembler,
# not a CodeGen TargetMachine. Registering a TargetMachine (+ Subtarget,
# RegisterInfo, FrameLowering, ISelLowering, ISelDAGToDAG, AsmPrinter) is the
# remaining milestone-3b work; see plans/implementation_plan.md and
# report/final_report.md "Milestone 3b". This script is the reproducible
# acceptance harness for when that backbone lands.
set -uo pipefail

BIN="${I4004_BIN:-/workspace/llvm-i4004/install/bin}"
NIGHTLY="${RUST_NIGHTLY:-nightly-2026-10-15}"
LAYOUT='e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8'   # abi.md §1
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
export PATH="$BIN:$PATH"

cat > "$WORK/bcd_add.rs" <<'RS'
#![no_std]
#![crate_type = "lib"]
#[panic_handler]
fn ph(_: &core::panic::PanicInfo) -> ! { loop {} }

pub const DIGITS: usize = 16;

/// Innermost BCD add (no memory): the single-digit core of bcd.rs add_mag.
#[no_mangle]
pub extern "C" fn bcd_add_digit(x: u8, y: u8, cin: u8) -> u16 {
    let mut s = x + y + cin;
    let mut c = 0u8;
    if s > 9 { s -= 10; c = 1; }
    ((c as u16) << 8) | (s as u16)
}

/// a[0..16] += b[0..16] as 16-digit BCD; returns 1 if no carry out.
/// Transcribed from firmware/src/bcd.rs `add_mag` (one loop, free function).
#[no_mangle]
pub extern "C" fn bcd_add_mag(a: &mut [u8; DIGITS], b: &[u8; DIGITS]) -> u8 {
    let mut carry: u8 = 0;
    let mut i = 0;
    while i < DIGITS {
        let mut s = a[i] + b[i] + carry;
        carry = 0;
        if s > 9 { s -= 10; carry = 1; }
        a[i] = s;
        i += 1;
    }
    if carry == 0 { 1 } else { 0 }
}
RS

echo "==> emitting IR with rustc +$NIGHTLY (LLVM must be 23.1.x)"
rustc "+$NIGHTLY" --emit=llvm-ir --crate-type=lib -C opt-level=z -C panic=abort \
    -o "$WORK/bcd_add.ll" "$WORK/bcd_add.rs" || { echo "rustc failed"; exit 2; }

echo "==> adapting data layout + triple to i4004 (abi.md §1)"
sed -E "s|^target datalayout = .*|target datalayout = \"$LAYOUT\"|; \
        s|^target triple = .*|target triple = \"i4004\"|" \
    "$WORK/bcd_add.ll" > "$WORK/bcd_i4004.ll"
head -4 "$WORK/bcd_i4004.ll"

echo "==> llc -mtriple=i4004 (compile to asm)"
if llc -mtriple=i4004 "$WORK/bcd_i4004.ll" -o "$WORK/bcd.s" 2>"$WORK/err"; then
    echo "==> llc produced assembly:"
    cat "$WORK/bcd.s"
    echo "==> assembling with our llvm-mc"
    if llvm-mc -triple=i4004 --filetype=obj "$WORK/bcd.s" -o "$WORK/bcd.o" 2>>"$WORK/err"; then
        SIZE=$(llvm-objdump -h "$WORK/bcd.o" | awk '/\.text/{print strtonum("0x"$3)}')
        echo "==> SUCCESS: .text size = ${SIZE:-?} bytes"
    else
        echo "==> llvm-mc rejected the asm:"; cat "$WORK/err"
    fi
else
    echo "==> BLOCKER: llc could not compile. Error:"
    cat "$WORK/err"
    echo
    echo "This is expected until the milestone-3b CodeGen backbone (TargetMachine,"
    echo "Subtarget, RegisterInfo, FrameLowering, ISelLowering, ISelDAGToDAG,"
    echo "AsmPrinter) is built. The MC layer + disassembler alone register no"
    echo "llc target. See report/final_report.md 'Milestone 3b'."
    exit 1
fi
