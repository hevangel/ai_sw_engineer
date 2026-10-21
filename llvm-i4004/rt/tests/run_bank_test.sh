#!/usr/bin/env bash
# Execution-test the ROM bank latch + a cross-bank call on chips.
# Builds a 2-bank image:
#   COMMON (chips #0-#3): reset -> call trampoline -> store result -> halt.
#   The trampoline switches to bank 1, JMS a function at a fixed banked address,
#   switches back to bank 0, returns.
#   BANK 0 high (chips #4..): fn@0x400 returns 0xA (LDM 0xA in P1).
#   BANK 1 high: fn@0x400 returns 0xB.
# A correct cross-bank call returns 0xB (the bank-1 function), proving the latch
# routes fetches to the selected bank and the trampoline restores bank 0.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/llvm-build/llvm-obj/bin"; [[ -x "$B/llvm-mc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/bank.XXXXXX")}"
mkdir -p "$WORK"
fail(){ echo "FAIL: $*" >&2; exit 1; }

# harness (bank_test.rs)
mkdir -p "$WORK/h/src"
cat > "$WORK/h/Cargo.toml" <<EOF
[package]
name="bank-test"
version="0.1.0"
edition="2021"
[[bin]]
name="bank_test"
path="src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cp "$RT/tests/bank_test.rs" "$WORK/h/src/main.rs"
( cd "$WORK/h" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/c.log" 2>&1 ) || { tail -25 "$WORK/c.log"; fail "harness build"; }
HARNESS="$WORK/ct/release/bank_test"

# --- COMMON block (chips 0-3, placed at 0x000) ---------------------------
# reset@0: JUN start. start: SSP=0x40; JMS tramp; store P1 result to RAM0/reg0
# char0; halt. tramp: set latch=1 (SRC chip3; LDM1; WRR); JMS 0x400; set latch=0;
# BBL. We hand-place with absolute .org via the packer's section addressing; to
# keep it simple we assemble one flat section and rely on JUN/JMS absolute addrs.
cat > "$WORK/common.s" <<'EOF'
	.section .reset,"ax"
reset:	jun start
	.section .text.start,"ax"
start:
	fim	p7, 0x40
	jms	tramp
	fim	p0, 0x00
	src	p0
	ld	r3
	wrm
halt:	jun halt
	.section .text.tramp,"ax"
tramp:
	fim	p0, 0x30	; SRC addr: ROM chip 3 (high nibble = 3)
	src	p0
	ldm	1		; bank 1
	wrr
	jms	target		; target is at 0x400 in the banked region
	fim	p0, 0x30
	src	p0
	ldm	0		; back to bank 0
	wrr
	bbl	0
EOF
# target symbol is resolved to 0x400 by the linker via the banked object.

# --- BANKED blocks: a function `target` at banked offset 0 (address 0x400) ---
# bank 0 returns 0xA, bank 1 returns 0xB. We build each as its own object whose
# single .text.target section the packer places at 0x400 (first banked chip).
mk_bank(){ # <val> <out.o>
	cat > "$WORK/bk.s" <<EOF
	.section .text.target,"ax"
	.globl target
target:
	fim	p1, 0x0$1
	bbl	0
EOF
	$B/llvm-mc -triple i4004 --filetype=obj "$WORK/bk.s" -o "$2" 2>"$WORK/mc.log" || { cat "$WORK/mc.log"; fail "asm bank $1"; }
}
$B/llvm-mc -triple i4004 --filetype=obj "$WORK/common.s" -o "$WORK/common.o" 2>"$WORK/mc.log" || { cat "$WORK/mc.log"; fail "asm common"; }
mk_bank a "$WORK/b0.o"
mk_bank b "$WORK/b1.o"

# Link the COMMON block + bank0 target into a 4 KiB image (bank 0 view): common
# at 0x000, target at 0x400. We use the packer with an explicit ROM layout.
# Produce common(0x400) and each bank_hi(0xC00) separately, then concatenate.
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
# Link bank 0: common + b0 -> full 0x1000 image; carve common[0..0x400] and
# bank-hi[0x400..0x1000].
python3 "$PACK" "$WORK/common.o" "$WORK/b0.o" -o "$WORK/full0.bin" --max 4096 --pad 4096 >/dev/null 2>"$WORK/p0.log" || { cat "$WORK/p0.log"; fail "link bank0"; }
python3 "$PACK" "$WORK/common.o" "$WORK/b1.o" -o "$WORK/full1.bin" --max 4096 --pad 4096 >/dev/null 2>"$WORK/p1.log" || { cat "$WORK/p1.log"; fail "link bank1"; }
# image = common(0x400 from full0) || bank0_hi(0xC00 from full0) || bank1_hi(0xC00 from full1)
python3 - "$WORK/full0.bin" "$WORK/full1.bin" "$WORK/img.bin" <<'PY'
import sys
f0=open(sys.argv[1],'rb').read(); f1=open(sys.argv[2],'rb').read()
common=f0[:0x400]; b0=f0[0x400:0x1000]; b1=f1[0x400:0x1000]
open(sys.argv[3],'wb').write(common+b0+b1)
PY

got=$("$HARNESS" "$WORK/img.bin" 4000 1)
if [[ "$got" == "11" ]]; then
  echo "BANK RESULT: PASS (cross-bank call returned 0xB=11 from bank 1)" >&2
else
  echo "BANK RESULT: got [$got] want [11] (0xB)" >&2
  echo "(0xA=10 would mean the latch did not switch; the trampoline/latch is wrong)" >&2
  exit 1
fi
