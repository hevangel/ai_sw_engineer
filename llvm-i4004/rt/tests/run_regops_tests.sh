#!/usr/bin/env bash
# Execution-test the regops block primitives on external/chips vs a host oracle.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
RT="$ROOT/llvm-i4004/rt"
B="/llvm-build/llvm-obj/bin"
[[ -x "$B/llvm-mc" ]] || B="$ROOT/llvm-i4004/install/bin"
CHIPS_DIR="${CHIPS_DIR:-$ROOT/external/chips}"
WORK="${WORK:-$(mktemp -d "${KIROCREW_SCRATCH:-/tmp}/regops.XXXXXX")}"
PACK="$ROOT/llvm-i4004/asm/i4004_pack.py"
GEN="$RT/tests/gen_regops_test.py"
mkdir -p "$WORK"
fail(){ echo "FAIL: $*" >&2; exit 1; }

# build harness once (reuse rt_test.rs)
mkdir -p "$WORK/rt_test/src"
cat > "$WORK/rt_test/Cargo.toml" <<EOF
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
cp "$RT/tests/rt_test.rs" "$WORK/rt_test/src/main.rs"
( cd "$WORK/rt_test" && CARGO_TARGET_DIR="$WORK/ct" cargo build --release >"$WORK/cargo.log" 2>&1 ) || { tail -20 "$WORK/cargo.log"; fail "harness build"; }
HARNESS="$WORK/ct/release/rt_test"
$B/llvm-mc -triple i4004 --filetype=obj "$RT/regops.s" -o "$WORK/regops.o" 2>"$WORK/mc.log" || { cat "$WORK/mc.log"; fail "assemble regops.s"; }

pass=0; failc=0
run(){ # <sym> <aCSV> <bCSV> <ret:0|1> <nprobe> <expectedCSV>
  local sym="$1" a="$2" b="$3" ret="$4" np="$5" exp="$6"
  local flags="--a $a --b $b --nprobe $np"; [[ "$ret" == 1 ]] && flags="$flags --ret"
  python3 "$GEN" "$sym" $flags > "$WORK/s.s"
  $B/llvm-mc -triple i4004 --filetype=obj "$WORK/s.s" -o "$WORK/s.o" 2>"$WORK/mc.log" || { cat "$WORK/mc.log"; fail "asm stub $sym"; }
  python3 "$PACK" "$WORK/s.o" "$WORK/regops.o" -o "$WORK/img.bin" --pad-pages --pad 1280 >/dev/null 2>"$WORK/pk.log" || { cat "$WORK/pk.log"; fail "link $sym"; }
  local n; [[ "$ret" == 1 ]] && n=1 || n=$np
  local got; got=$("$HARNESS" "$WORK/img.bin" 20000 "$n")
  # truncate expected to the probed digit count
  local want; want=$(echo "$exp" | cut -d" " -f1-"$n")
  if [[ "$got" == "$want" ]]; then pass=$((pass+1)); else failc=$((failc+1)); echo "MISMATCH $sym a=[$a] b=[$b]: got [$got] want [$want]" >&2; fi
}

# oracle helpers in python (print space-separated digits)
orc(){ python3 "$RT/tests/regops_oracle.py" "$@"; }

echo "== reg_copy ==" >&2
A="1,2,3,4,5,6,7,8,9,0,1,2,3,4,5,6"; Bz="0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0"
run reg_copy "$Bz" "$A" 0 8 "$(orc copy $A)"
echo "== reg_clear ==" >&2
run reg_clear "$A" "$Bz" 0 8 "$(orc clear)"
echo "== reg_is_zero ==" >&2
run reg_is_zero "$Bz" "$Bz" 1 1 "1"
run reg_is_zero "$A" "$Bz" 1 1 "0"
echo "== reg_add ==" >&2
# little-endian digits: A=...(digit0 first). 9+9 chains to test carry.
run reg_add "9,9,9,0,0,0,0,0,0,0,0,0,0,0,0,0" "1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0" 0 4 "$(orc add 9,9,9 1)"
run reg_add "5,5,5,5,0,0,0,0,0,0,0,0,0,0,0,0" "5,5,5,5,0,0,0,0,0,0,0,0,0,0,0,0" 0 5 "$(orc add 5,5,5,5 5,5,5,5)"
echo "== reg_sub ==" >&2
run reg_sub "0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0" "1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0" 0 4 "$(orc sub 0,0,1 1)"
echo "== reg_cmp ==" >&2
run reg_cmp "1,2,3,0,0,0,0,0,0,0,0,0,0,0,0,0" "1,2,3,0,0,0,0,0,0,0,0,0,0,0,0,0" 1 1 "1"
run reg_cmp "1,2,4,0,0,0,0,0,0,0,0,0,0,0,0,0" "1,2,3,0,0,0,0,0,0,0,0,0,0,0,0,0" 1 1 "2"
run reg_cmp "1,2,2,0,0,0,0,0,0,0,0,0,0,0,0,0" "1,2,3,0,0,0,0,0,0,0,0,0,0,0,0,0" 1 1 "0"

echo "" >&2
echo "REGOPS RESULT: $pass passed, $failc failed" >&2
[[ "$failc" == 0 ]]
