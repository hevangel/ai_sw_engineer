#!/usr/bin/env bash
# Execution-test far-branch relaxation: build a function LARGER than a 256-byte
# ROM page whose conditional branches (loop back-edge + compares) therefore must
# span pages, link WITHOUT --pad-pages, run on chips, and check the result.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/llvm-build/llvm-obj/bin"; [[ -x "$B/llc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/relax.XXXXXX")}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
mkdir -p "$WORK"
fail(){ echo "FAIL: $*" >&2; exit 1; }

# harness
mkdir -p "$WORK/h/src"
cat > "$WORK/h/Cargo.toml" <<EOF
[package]
name="rt-test"
version="0.1.0"
edition="2021"
[[bin]]
name="rt_test"
path="src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cp "$RT/tests/rt_test.rs" "$WORK/h/src/main.rs"
( cd "$WORK/h" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/c.log" 2>&1 ) || { tail -20 "$WORK/c.log"; fail "harness"; }
HARNESS="$WORK/ct/release/rt_test"

DL='target datalayout = "e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8"'
# A deliberately LARGE function: sum 1..=n with a loop (ISZ/compare back-edge),
# then a long straight-line tail of i8 adds to push it past 256 bytes so the
# loop's branches straddle a page. Returns sum(1..=n) + K (mod 256). n=5 -> 15.
# We make it big by a chain of volatile-ish adds the optimiser keeps.
{
  echo "$DL"
  echo 'target triple = "i4004"'
  echo 'define i8 @big(ptr %p, i8 %n) {'
  echo 'entry: br label %loop'
  echo 'loop:'
  echo '  %i = phi i8 [1, %entry], [%ni, %body]'
  echo '  %acc = phi i8 [0, %entry], [%na, %body]'
  echo '  %done = icmp ugt i8 %i, %n'
  echo '  br i1 %done, label %tail, label %body'
  echo 'body:'
  echo '  %na = add i8 %acc, %i'
  echo '  store volatile i8 %na, ptr %p'
  echo '  %ni = add i8 %i, 1'
  echo '  br label %loop'
  echo 'tail:'
  # long straight-line tail of volatile stores so the optimiser CANNOT fold it;
  # this keeps big() well over a 256-byte page, forcing the loop branches to
  # straddle a page (the thing under test).
  prev="%acc"
  for k in $(seq 1 20); do
    echo "  %t$k = add i8 $prev, $k"
    echo "  store volatile i8 %t$k, ptr %p"
    prev="%t$k"
  done
  echo "  ret i8 $prev"
  echo '}'
} > "$WORK/big.ll"

# stub: call big(&cell, 5); cell at RAM0/reg0 char 8 (0x08). Result -> char0/1.
cat > "$WORK/stub.s" <<'EOF'
	.section .reset,"ax"
reset:	jun start
	.text
start:
	fim	p1, 0x05
	fim	p2, 0x08
	fim	p3, 0x00
	jms	big
	fim	p0, 0x00
	src	p0
	ld	r3
	wrm
	fim	p0, 0x01
	src	p0
	ld	r2
	wrm
halt:	jun halt
EOF

timeout 40 "$B/llc" -mtriple=i4004 -function-sections -filetype=obj "$WORK/big.ll" -o "$WORK/big.o" 2>"$WORK/llc.log" || { cat "$WORK/llc.log"; fail "llc big"; }
# report big's size
BIG=$("$B/llvm-readobj" --sections "$WORK/big.o" 2>/dev/null | awk '/Name: .text\./{n=1} n&&/Size:/{print $2; n=0}' | head -1)
echo "big() .text = $BIG bytes (page = 256)" >&2
timeout 40 "$B/llvm-mc" -triple i4004 --filetype=obj "$WORK/stub.s" -o "$WORK/stub.o" 2>"$WORK/mc.log" || { cat "$WORK/mc.log"; fail "asm stub"; }
# link WITHOUT --pad-pages, 4096 budget (single ROM span); big + stub < 4096.
python3 "$PACK" "$WORK/stub.o" "$WORK/big.o" -o "$WORK/img.bin" -m "$WORK/img.map" --max 4096 --pad 1280 >/dev/null 2>"$WORK/pk.log" || { cat "$WORK/pk.log"; fail "link (relaxation should make this succeed)"; }

# host oracle: sum(1..=5)=15; tail adds 1..120 => +sum(1..120)=7260; total mod 256
exp=$(python3 -c 'print((15 + sum(range(1,21))) & 0xff)')
elo=$((exp & 0xf)); ehi=$(((exp>>4)&0xf))
got=$("$HARNESS" "$WORK/img.bin" 60000 2)
if [[ "$got" == "$elo $ehi" ]]; then
  echo "RELAX RESULT: PASS (big()=$BIG B spans pages; result $got == $elo $ehi)" >&2
else
  echo "RELAX RESULT: FAIL got [$got] want [$elo $ehi]" >&2; exit 1
fi
