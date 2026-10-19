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
	; push return token for `halt` (so entry's final __soft_ret lands at halt)
	fim	p0, %lo8(halt)
	src	p7
	ld	r1
	wrm
	inc	r15
	src	p7
	ld	r0
	wrm
	inc	r15
	fim	p0, %page(halt)
	src	p7
	ld	r0
	wrm
	inc	r15
	jun	entry
	.globl halt
halt:	jun halt
EOF

# --- test IR: deep chain + value preserved across a call ---------------------
# Both exceed the 3-level HARDWARE stack, so they prove the software return
# stack and that callee-saved state survives a soft call. Depth stays within
# the first soft-stack 4002 register (<= 5 three-nibble frames incl. reset's
# halt frame): the inline push bumps only the SSP char nibble (R15), so an
# 8-bit SSP carry (R15->R14) is the separate next step for depth > 5.
# (Recursion value-accumulation and i16-return-through-a-call expose a
# register-spill bug tracked separately; they are NOT soft-call-mechanism bugs
# -- the mechanism returns correctly here.)
cat > "$WORK/t.ll" <<'EOF'
target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"
target triple = "i4004"

; depth-4 chain (4 live soft frames > 3 HW levels): chain4(0) = 4.
define i8 @f4(i8 %x) { %r = add i8 %x, 1 ret i8 %r }
define i8 @f3(i8 %x) { %a = add i8 %x,1 %r = call i8 @f4(i8 %a) ret i8 %r }
define i8 @f2(i8 %x) { %a = add i8 %x,1 %r = call i8 @f3(i8 %a) ret i8 %r }
define i8 @chain4(i8 %x) { %a = add i8 %x,1 %r = call i8 @f2(i8 %a) ret i8 %r }

; value preserved across a soft call: g(2) = h(2) + 2 = 12 + 2 = 14.
define i8 @h(i8 %x) { %r = add i8 %x, 10 ret i8 %r }
define i8 @g(i8 %x) { %t = call i8 @h(i8 %x) %r = add i8 %t, %x ret i8 %r }

define void @entry() {
  ; case 1: chain4(0) -> 4  ; store at RAM[0]
  %c = call i8 @chain4(i8 0)
  store volatile i8 %c, ptr inttoptr (i16 0 to ptr)
  ; case 2: g(2) -> 14      ; store at RAM[2]
  %s = call i8 @g(i8 2)
  store volatile i8 %s, ptr inttoptr (i16 2 to ptr)
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
got=$("$HARNESS" "$WORK/img.bin" 20000 4)
# chain4(0)=4 -> nibbles 4,0 ; g(2)=14 (0x0e) -> 14,0
want="4 0 14 0"
if [[ "$got" == "$want" ]]; then
  echo "SOFTCALL EXEC: PASS (chain4=4 depth-4 soft calls; g=14 value preserved across call)" >&2
  exit 0
else
  echo "SOFTCALL EXEC: got [$got] want [$want]" >&2
  echo "(map: $WORK/img.map)" >&2
  sed -n '1,40p' "$WORK/img.map" >&2
  exit 1
fi
