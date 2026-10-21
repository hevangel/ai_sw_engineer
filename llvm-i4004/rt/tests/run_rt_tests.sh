#!/usr/bin/env bash
# libi4004rt helper execution tests on `chips`.
#
# Runs ENTIRELY inside the ai-sw-engineer container (python3 + cargo + the
# llvm-mc toolchain on the mounted ai-sw-llvm-build volume). It:
#   1. builds the bare-CPU rt_test harness once (cargo, path-dep on external/chips);
#   2. for each helper and each input pair, generates a test stub
#      (gen_rt_test.py), assembles stub + helper sources (llvm-mc), links a ROM
#      image (i4004_pack.py), runs it on `chips` (rt_test), reads back the result
#      nibbles, and compares them against a host reference computed here in
#      python -- an INDEPENDENT oracle, never the helper's own output.
#
# Invoke from the repo root, e.g.:
#   docker run --rm -v ai-sw-llvm-build:/llvm-build -v $PWD:/ws -w /ws \
#     ai-sw-engineer:latest bash llvm-i4004/rt/tests/run_rt_tests.sh [quick|full]
#
# MODE: quick (default) = a representative sample; full = exhaustive 8-bit where
# feasible (mul/div/mod over a stride, shifts/logic exhaustive).
set -euo pipefail

MODE="${1:-quick}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/llvm-build/llvm-obj/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/rt_test.XXXXXX")}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
GEN="$RT/tests/gen_rt_test.py"

mkdir -p "$WORK"
status(){ printf '==> %s\n' "$*" >&2; }
fail(){ printf 'FAIL: %s\n' "$*" >&2; exit 1; }

[[ -x "$B/llvm-mc" ]] || fail "llvm-mc missing at $B (mount ai-sw-llvm-build)"
[[ -f "$CHIPS_DIR/src/mcs4/cpu4004.rs" ]] || fail "external/chips not at $CHIPS_DIR"

# ---- build the rt_test harness once ----------------------------------------
status "building rt_test harness (cargo, path-dep on chips)"
mkdir -p "$WORK/rt_test/src"
cat > "$WORK/rt_test/Cargo.toml" <<EOF
[package]
name = "rt-test"
version = "0.1.0"
edition = "2021"
[[bin]]
name = "rt_test"
path = "src/main.rs"
[dependencies]
chips = { path = "$CHIPS_DIR" }
arbitrary-int = "=1.3.0"
EOF
cp "$RT/tests/rt_test.rs" "$WORK/rt_test/src/main.rs"
( cd "$WORK/rt_test" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/cargo.log" 2>&1 ) \
    || { tail -30 "$WORK/cargo.log" >&2; fail "rt_test build failed"; }
HARNESS="$WORK/ct/release/rt_test"

# cycles to run per image: the stubs are tiny; 4000 cycles is ample for an 8-bit
# mul/div (each ~8 iterations x ~20 instr) plus the stores.
CYCLES=4000

pass=0; failc=0

# run_case <helper_src_list> <helper_sym> <a> <b> <two_out:0|1> <exp0> <exp1> [exp2 exp3]
run_case(){
  local srcs="$1" sym="$2" a="$3" b="$4" two="$5"; shift 5
  local exp=("$@")
  local stub="$WORK/stub.s" img="$WORK/img.bin"
  local two_flag=""; [[ "$two" == 1 ]] && two_flag="--two-out"
  python3 "$GEN" "$sym" --a "$a" --b "$b" $two_flag > "$stub"
  # assemble stub + helper sources
  local objs=()
  $B/llvm-mc -triple i4004 --filetype=obj "$stub" -o "$WORK/stub.o" 2>"$WORK/mc.log" \
      || { cat "$WORK/mc.log" >&2; fail "assemble stub ($sym a=$a b=$b)"; }
  objs+=("$WORK/stub.o")
  local i=0
  for s in $srcs; do
    $B/llvm-mc -triple i4004 --filetype=obj "$RT/$s" -o "$WORK/h$i.o" 2>"$WORK/mc.log" \
        || { cat "$WORK/mc.log" >&2; fail "assemble $s"; }
    objs+=("$WORK/h$i.o"); i=$((i+1))
  done
  python3 "$PACK" "${objs[@]}" -o "$img" --pad-pages --pad 1280 >/dev/null 2>"$WORK/pk.log" \
      || { cat "$WORK/pk.log" >&2; fail "link ($sym a=$a b=$b)"; }
  local nchars=$(( two == 1 ? 4 : 2 ))
  local got; got=$("$HARNESS" "$img" "$CYCLES" "$nchars")
  local want="${exp[*]}"
  if [[ "$got" == "$want" ]]; then
    pass=$((pass+1))
  else
    failc=$((failc+1))
    echo "MISMATCH $sym a=$a b=$b: got [$got] want [$want]" >&2
  fi
}

# reference helpers (python, host oracle)
lo(){ echo $(( $1 & 0xf )); }
hi(){ echo $(( ($1 >> 4) & 0xf )); }

status "=== __mulqi3 (i8*i8 -> low 8 bits) ==="
if [[ "$MODE" == full ]]; then AS="$(seq 0 7 255)"; BS="$(seq 0 11 255)"; else AS="0 1 3 7 9 12 15 16 100 200 255"; BS="0 1 2 5 10 13 17 25 99 128 255"; fi
for a in $AS; do for b in $BS; do
  p=$(( (a*b) & 0xff ))
  run_case "arith.s" __mulqi3 "$a" "$b" 0 "$(lo $p)" "$(hi $p)"
done; done

status "=== __udivqi3 / __umodqi3 ==="
if [[ "$MODE" == full ]]; then AS="$(seq 0 5 255)"; BS="1 2 3 5 7 10 13 16 100 255"; else AS="0 1 7 10 100 200 255"; BS="1 2 3 7 10 16 100 255"; fi
for a in $AS; do for b in $BS; do
  [[ "$b" == 0 ]] && continue
  q=$(( a / b )); r=$(( a % b ))
  run_case "arith.s" __udivqi3 "$a" "$b" 0 "$(lo $q)" "$(hi $q)"
  run_case "arith.s" __umodqi3 "$a" "$b" 0 "$(lo $r)" "$(hi $r)"
done; done

status "=== __negqi2 (two's complement) ==="
for a in 0 1 2 15 16 100 127 128 200 255; do
  n=$(( (256 - a) & 0xff ))
  run_case "arith.s" __negqi2 "$a" 0 0 "$(lo $n)" "$(hi $n)"
done

status "=== shifts: __ashlqi3 / __lshrqi3 / __ashrqi3 ==="
if [[ -f "$RT/shift.s" ]]; then
  for a in 0 1 2 5 0x55 0xAA 0x80 0x7F 0xFF; do for n in 0 1 2 3 4 7 8; do
    av=$(( a & 0xff ))
    shl=$(( (av << n) & 0xff ))
    lsr=$(( (av >> n) & 0xff ))
    # arithmetic right: sign-extend bit7
    if (( av & 0x80 )); then asr=$(( ( (av | 0xffffff00) >> n ) & 0xff )); else asr=$(( av >> n )); fi
    run_case "shift.s" __ashlqi3 "$av" "$n" 0 "$(lo $shl)" "$(hi $shl)"
    run_case "shift.s" __lshrqi3 "$av" "$n" 0 "$(lo $lsr)" "$(hi $lsr)"
    run_case "shift.s" __ashrqi3 "$av" "$n" 0 "$(lo $asr)" "$(hi $asr)"
  done; done
fi

status "=== logic: __andqi3 / __orqi3 / __xorqi3 ==="
if [[ -f "$RT/logic.s" ]]; then
  LSRC="logic.s"
  for a in 0 0x0F 0xF0 0x55 0xAA 0xFF 0x3C 0x81; do for b in 0 0x0F 0xF0 0x55 0xAA 0xFF 0x12 0x7E; do
    av=$(( a & 0xff )); bv=$(( b & 0xff ))
    an=$(( av & bv )); orr=$(( av | bv )); xr=$(( av ^ bv ))
    run_case "$LSRC" __andqi3 "$av" "$bv" 0 "$(lo $an)" "$(hi $an)"
    run_case "$LSRC" __orqi3  "$av" "$bv" 0 "$(lo $orr)" "$(hi $orr)"
    run_case "$LSRC" __xorqi3 "$av" "$bv" 0 "$(lo $xr)" "$(hi $xr)"
  done; done
fi

# --- 16-bit (hi3) helpers: args in D1/D2, i16 result in D1 -------------------
# nib k of value v (0=low .. 3=high); stored order is char0=byte0.lo,
# char1=byte0.hi, char2=byte1.lo, char3=byte1.hi.
nib(){ echo $(( ($1 >> (4*$2)) & 0xf )); }

# run_wide <helper_src_list> <sym> <a16> <b16> <exp16>
run_wide(){
  local srcs="$1" sym="$2" a="$3" b="$4" exp="$5"
  local stub="$WORK/stub.s" img="$WORK/img.bin"
  python3 "$GEN" "$sym" --a "$a" --b "$b" --wide > "$stub"
  local objs=()
  $B/llvm-mc -triple i4004 --filetype=obj "$stub" -o "$WORK/stub.o" 2>"$WORK/mc.log" \
      || { cat "$WORK/mc.log" >&2; fail "assemble wide stub ($sym a=$a b=$b)"; }
  objs+=("$WORK/stub.o")
  local i=0
  for s in $srcs; do
    $B/llvm-mc -triple i4004 --filetype=obj "$RT/$s" -o "$WORK/hw$i.o" 2>"$WORK/mc.log" \
        || { cat "$WORK/mc.log" >&2; fail "assemble $s"; }
    objs+=("$WORK/hw$i.o"); i=$((i+1))
  done
  python3 "$PACK" "${objs[@]}" -o "$img" --pad-pages --pad 1280 >/dev/null 2>"$WORK/pk.log" \
      || { cat "$WORK/pk.log" >&2; fail "link wide ($sym a=$a b=$b)"; }
  local got; got=$("$HARNESS" "$img" "$CYCLES" 4)
  local want="$(nib $exp 0) $(nib $exp 1) $(nib $exp 2) $(nib $exp 3)"
  if [[ "$got" == "$want" ]]; then
    pass=$((pass+1))
  else
    failc=$((failc+1))
    echo "MISMATCH $sym a=$a b=$b: got [$got] want [$want]" >&2
  fi
}

status "=== logic16: __andhi3 / __orhi3 / __xorhi3 (i16) ==="
if [[ -f "$RT/logic16.s" ]]; then
  for a in 0 0x00FF 0xFF00 0x5555 0xAAAA 0xFFFF 0x1234 0x8001 0x0F0F; do
    for b in 0 0x00FF 0xFF00 0x5555 0xAAAA 0xFFFF 0xABCD 0x8000 0xF0F0; do
      av=$(( a & 0xffff )); bv=$(( b & 0xffff ))
      an=$(( av & bv )); orr=$(( av | bv )); xr=$(( av ^ bv ))
      run_wide "logic16.s" __andhi3 "$av" "$bv" "$an"
      run_wide "logic16.s" __orhi3  "$av" "$bv" "$orr"
      run_wide "logic16.s" __xorhi3 "$av" "$bv" "$xr"
    done
  done
fi

status "=== shift16: __ashlhi3 / __lshrhi3 / __ashrhi3 (i16) ==="
if [[ -f "$RT/shift16.s" ]]; then
  for a in 0 1 0x00FF 0x0100 0x1234 0x8001 0xAAAA 0xFFFF; do
    for n in 0 1 2 3 4 7 8 12 15 16 20; do
      av=$(( a & 0xffff ))
      shl=$(( (av << n) & 0xffff ))
      lsr=$(( av >> n )); (( n >= 16 )) && lsr=0
      if (( av & 0x8000 )); then
        if (( n >= 16 )); then asr=0xffff; else asr=$(( ( (av | 0xffff0000) >> n ) & 0xffff )); fi
      else
        asr=$(( av >> n )); (( n >= 16 )) && asr=0
      fi
      run_wide "shift16.s" __ashlhi3 "$av" "$n" "$shl"
      run_wide "shift16.s" __lshrhi3 "$av" "$n" "$lsr"
      run_wide "shift16.s" __ashrhi3 "$av" "$n" "$asr"
    done
  done
fi

status "=== arith16: __mulhi3 / __udivhi3 / __umodhi3 (i16) ==="
if [[ -f "$RT/arith16.s" ]]; then
  # arith16 uses the soft stack (P7); the wide stub inits SSP to 0x40. The
  # divider links the stack helper (stack.s) too.
  if [[ "$MODE" == full ]]; then
    MAS="0 1 2 10 100 255 256 1000 4096 12345 40000 65535"
    MBS="1 2 3 7 10 100 256 1000 9999 65535"
  else
    MAS="0 1 7 100 255 1000 12345 40000 65535"
    MBS="1 2 3 10 100 256 1000 65535"
  fi
  for a in $MAS; do for b in $MBS; do
    av=$(( a & 0xffff )); bv=$(( b & 0xffff ))
    mul=$(( (av * bv) & 0xffff ))
    run_wide "arith16.s" __mulhi3 "$av" "$bv" "$mul"
    [[ "$bv" == 0 ]] && continue
    q=$(( av / bv )); r=$(( av % bv ))
    run_wide "arith16.s" __udivhi3 "$av" "$bv" "$q"
    run_wide "arith16.s" __umodhi3 "$av" "$bv" "$r"
  done; done
fi

status "=== mem: __memset_ram / __memcpy_ram across a 4002 register boundary ==="
MEMGEN="$RT/tests/gen_mem_test.py"
# run_mem <mode> <addr> <extra:val|src> <count> <nprobe> <probe_base> <expected nprobe nibbles...>
run_mem(){
  local mode="$1" addr="$2" extra="$3" count="$4" nprobe="$5" pbase="$6"; shift 6
  local exp=("$@")
  local stub="$WORK/memstub.s" img="$WORK/memimg.bin"
  if [[ "$mode" == memset ]]; then
    python3 "$MEMGEN" memset --addr "$addr" --val "$extra" --count "$count" --nprobe "$nprobe" --probe-base "$pbase" > "$stub"
  else
    python3 "$MEMGEN" memcpy --addr "$addr" --src "$extra" --count "$count" --nprobe "$nprobe" --probe-base "$pbase" > "$stub"
  fi
  $B/llvm-mc -triple i4004 --filetype=obj "$stub" -o "$WORK/memstub.o" 2>"$WORK/mc.log" \
      || { cat "$WORK/mc.log" >&2; fail "assemble mem stub ($mode)"; }
  $B/llvm-mc -triple i4004 --filetype=obj "$RT/mem.s" -o "$WORK/mem.o" 2>"$WORK/mc.log" \
      || { cat "$WORK/mc.log" >&2; fail "assemble mem.s"; }
  python3 "$PACK" "$WORK/memstub.o" "$WORK/mem.o" -o "$img" --pad-pages --pad 1280 >/dev/null 2>"$WORK/pk.log" \
      || { cat "$WORK/pk.log" >&2; fail "link mem ($mode addr=$addr)"; }
  local got; got=$("$HARNESS" "$img" "$CYCLES" "$nprobe")
  local want="${exp[*]}"
  if [[ "$got" == "$want" ]]; then pass=$((pass+1));
  else failc=$((failc+1)); echo "MISMATCH mem $mode addr=$addr count=$count probe=$pbase: got [$got] want [$want]" >&2; fi
}

# memset 20 nibbles of 0x5 at 0x08 (spans reg0 chars 8-15 + reg1 chars 0-11);
# probe 0x08..0x0f (reg0 side of the fill) -> all 5.
run_mem memset 0x08 0x5 20 8 0x08  5 5 5 5 5 5 5 5
# same fill, probe 0x10..0x17 (reg1, PAST the 0x10 register boundary) -> all 5.
run_mem memset 0x08 0x3 20 8 0x10  3 3 3 3 3 3 3 3
# memcpy: seed src ramp at 0x20, copy 20 nibbles to 0x08; probe dst 0x08..0x0f
# -> ramp 0..7 (reg0 side).
run_mem memcpy 0x08 0x20 20 8 0x08  0 1 2 3 4 5 6 7
# same copy, probe 0x10..0x17 (dst nibbles 8..15, PAST the boundary) -> 8..15.
run_mem memcpy 0x08 0x20 20 8 0x10  8 9 10 11 12 13 14 15

echo "" >&2
status "RESULT: $pass passed, $failc failed"
[[ "$failc" == 0 ]] || exit 1
