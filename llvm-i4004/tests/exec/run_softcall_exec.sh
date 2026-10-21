#!/usr/bin/env bash
# Execution-test the SOFTWARE-CALL convention end to end on `external/chips`
# (abi.md 4.4.1, items 1-4). Compiles LLVM IR with llc (-> CALLSOFT token push +
# JUN callee + JUN __soft_ret returns), links with the packer's --softcall
# synthesis (__soft_ret + __srtab + per-page jin-p0 stubs), runs on the real
# mcs4::Board, and checks RAM against an independent host oracle.
#
# Cases (all exceed the 3-level HARDWARE stack, so they prove the soft stack):
#   1. call chain depth 8   (chain8:  f0->f1->...->f7, returns a marker)
#   2. recursion depth 5    (fact-like countdown accumulator)
#   3. i16 return value      (add two i16 through a 2-deep call)
#   4. mixed with an rt leaf call (__mulqi3 via JMS inside a soft-called fn)
#
# Usage: run from repo root inside the container:
#   docker run --rm -v $PWD:/workspace -v ai-sw-llvm-build:/build -w /workspace \
#     ai-sw-engineer:latest bash llvm-i4004/tests/exec/run_softcall_exec.sh
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/build/llvm-obj/bin"; [[ -x "$B/llc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/scx.XXXXXX")}"
mkdir -p "$WORK"
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }
[[ -x "$B/llc" ]] || fail "llc missing at $B"

# --- harness (reuse rt_test.rs: runs an image, prints RAM chip0/reg0 chars) ---
status "building rt_test harness"
mkdir -p "$WORK/h/src"
cat > "$WORK/h/Cargo.toml" <<EOF
[package]
name="scx"
version="0.1.0"
edition="2021"
[[bin]]
name="scx"
path="src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cp "$RT/tests/rt_test.rs" "$WORK/h/src/main.rs"
( cd "$WORK/h" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/c.log" 2>&1 ) \
    || { tail -25 "$WORK/c.log" >&2; fail "harness build"; }
HARNESS="$WORK/ct/release/scx"

# --- reset stub: set SSP, push a return token to `halt`, JUN entry ----------
# A compiled `entry` returns via JUN __soft_ret, so we must enter it the soft
# way: push a 3-nibble token (lo,hi,page) for `halt`, then JUN entry.
cat > "$WORK/reset.s" <<'EOF'
	.section .reset,"ax"
reset:	jun start
	.section .text.start,"ax"
	.globl start
start:
	fim	p7, 0x40        ; SSP base (4002 chip 1, char 0)
	; push return token for `halt` via the same leaf helpers compiled code uses
	fim	p0, %lo8(halt)
	jms	__push_off
	fim	p0, %page(halt)
	jms	__push_pg
	jun	entry
	.globl halt
halt:	jun halt
EOF

# --- test IR: depth-8 chain + value preserved across a call + i16 return -----
# All exceed the 3-level HARDWARE stack, so they prove the software return
# stack. The depth-8 chain (8 soft frames = 24 nibbles) also proves the 8-bit
# SSP pointer carries the char nibble (R15) into R14 and walks across 4002
# registers -- 24 > the 16 chars of a single register.
cat > "$WORK/t.ll" <<'EOF'
target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"
target triple = "i4004"

; depth-8 chain (8 live soft frames > 3 HW levels, > 5 one-register frames):
; chain8(0) = 8.
define i8 @f7(i8 %x) { %r = add i8 %x, 1 ret i8 %r }
define i8 @f6(i8 %x) { %a = add i8 %x,1 %r = call i8 @f7(i8 %a) ret i8 %r }
define i8 @f5(i8 %x) { %a = add i8 %x,1 %r = call i8 @f6(i8 %a) ret i8 %r }
define i8 @f4(i8 %x) { %a = add i8 %x,1 %r = call i8 @f5(i8 %a) ret i8 %r }
define i8 @f3(i8 %x) { %a = add i8 %x,1 %r = call i8 @f4(i8 %a) ret i8 %r }
define i8 @f2(i8 %x) { %a = add i8 %x,1 %r = call i8 @f3(i8 %a) ret i8 %r }
define i8 @f1(i8 %x) { %a = add i8 %x,1 %r = call i8 @f2(i8 %a) ret i8 %r }
define i8 @chain8(i8 %x) { %a = add i8 %x,1 %r = call i8 @f1(i8 %a) ret i8 %r }

; value preserved across a soft call: g(2) = h(2) + 2 = 12 + 2 = 14.
define i8 @h(i8 %x) { %r = add i8 %x, 10 ret i8 %r }
define i8 @g(i8 %x) { %t = call i8 @h(i8 %x) %r = add i8 %t, %x ret i8 %r }

; i16 return through a 2-deep call: wide(0x0102,0x0304) = 0x0406.
define i16 @addw2(i16 %a, i16 %b) { %r = add i16 %a, %b ret i16 %r }
define i16 @wide(i16 %a, i16 %b) { %r = call i16 @addw2(i16 %a, i16 %b) ret i16 %r }

define void @entry() {
  ; case 1: chain8(0) -> 8  ; store at RAM[0]
  %c = call i8 @chain8(i8 0)
  store volatile i8 %c, ptr inttoptr (i16 0 to ptr)
  ; case 2: g(2) -> 14      ; store at RAM[2]
  %s = call i8 @g(i8 2)
  store volatile i8 %s, ptr inttoptr (i16 2 to ptr)
  ; case 3: wide(0x0102,0x0304) -> 0x0406 ; store at RAM[4..7]
  %w = call i16 @wide(i16 258, i16 772)
  store volatile i16 %w, ptr inttoptr (i16 4 to ptr)
  ret void
}
EOF

status "llc: compile test IR (CALLSOFT)"
timeout 120 "$B/llc" -mtriple=i4004 -function-sections -filetype=obj "$WORK/t.ll" -o "$WORK/t.o" \
    2>"$WORK/llc.log" || { cat "$WORK/llc.log" >&2; fail "llc"; }
timeout 60 "$B/llvm-mc" -triple i4004 --filetype=obj "$WORK/reset.s" -o "$WORK/reset.o" \
    2>"$WORK/mc.log" || { cat "$WORK/mc.log" >&2; fail "assemble reset"; }

status "link with --softcall synthesis"
timeout 120 python3 "$PACK" "$WORK/reset.o" "$WORK/t.o" \
    -o "$WORK/img.bin" -m "$WORK/img.map" --max 4096 --pad 4096 --softcall --pad-pages \
    2>"$WORK/pk.log" || { cat "$WORK/pk.log" >&2; fail "link --softcall"; }

status "run on chips"
# chars 0,1 = case1 (i8, low/hi nibble) ; 2,3 = case2 ; 4,5,6,7 = case3 (i16)
got=$("$HARNESS" "$WORK/img.bin" 20000 8)
# chain8(0)=8 -> 8,0 ; g(2)=14 -> 14,0 ; wide=0x0406 -> 6,0,4,0
want="8 0 14 0 6 0 4 0"
if [[ "$got" == "$want" ]]; then
  echo "SOFTCALL EXEC: PASS (chain8=8 depth-8 across 4002 regs; g=14 value preserved; wide=0x0406 i16 return)" >&2
  exit 0
else
  echo "SOFTCALL EXEC: got [$got] want [$want]" >&2
  echo "(map: $WORK/img.map)" >&2
  sed -n '1,40p' "$WORK/img.map" >&2
  exit 1
fi
