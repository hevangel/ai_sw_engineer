#!/usr/bin/env bash
# Medium-model proof (issue #301 phase 1): builds the proof crate twice from one source,
#   tiny.com   - no padding, one segment (link_com.ld), near calls;
#   medium.exe - with ~95 KB of padding, every function far (cc 1000), packed into three code
#                segments, libcalls given a per-segment runtime copy, a separate 64 KiB far
#                data segment reached by inline far loads/stores;
# and prints what the host reference computes. Run each under PC DOS 2.00 (MartyPC, see
# plans/implementation_plan.md "Toolchain proof"); the screens must show the host's text.
#   bash plans/medium_model/scripts/build_proof.sh <out dir>
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../../../.." && pwd)"
BE="$ROOT/llvm-i8086"; PLAT="$ROOT/system/ibm_pc/platform"
OUT="$(mkdir -p "${1:?out dir}" && cd "$1" && pwd)"
LLC="$BE/install/bin/llc -mtriple=i8086 -O2 -function-sections -data-sections"
MC="$BE/install/bin/llvm-mc -triple=i8086 -filetype=obj"
python3 -I "$HERE/scripts/gen_pad.py" "$HERE/crate/src/pad.rs"
export CARGO_TARGET_DIR=/tmp/i8086-ld-target
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD=/tmp/i8086-ld-target/release/i8086-ld
unset CARGO_TARGET_DIR
(cd "$HERE/crate/host" && CARGO_TARGET_DIR="$OUT/host" cargo +1.99.1 run -q --release > "$OUT/host_tiny.txt" &&
  CARGO_TARGET_DIR="$OUT/host" cargo +1.99.1 run -q --release -- pad > "$OUT/host_medium.txt")
mkdir -p "$OUT/tiny" "$OUT/med/rt"
# tiny
CARGO_TARGET_DIR=/tmp/i8086-rust-build/mm-tiny bash "$BE/scripts/build_rust.sh" "$HERE/crate" medium "$OUT/tiny" >"$OUT/tiny/build.log" 2>&1
python3 -I "$HERE/scripts/ir_medium.py" "$OUT/tiny/medium.ll" "$OUT/tiny/near.ll" --far-intrinsics
$LLC -filetype=obj "$OUT/tiny/near.ll" -o "$OUT/tiny/near.o"
$MC "$HERE/scripts/hal_near.s" -o "$OUT/tiny/hal_near.o"
"$LD" -T "$PLAT/link_com.ld" -o "$OUT/tiny/t.elf" -Map "$OUT/tiny/t.map" --com "$OUT/tiny.com" \
  "$PLAT/runtime/crt0_com.o" "$OUT/tiny/hal_near.o" "$OUT/tiny/near.o" "$BE"/rt/obj/*.o
# medium: near-model sizes first, then pack into segments
CARGO_TARGET_DIR=/tmp/i8086-rust-build/mm-med bash "$BE/scripts/build_rust.sh" "$HERE/crate" medium "$OUT/med" --features pad >"$OUT/med/build.log" 2>&1
python3 -I "$HERE/scripts/ir_medium.py" "$OUT/med/medium.ll" "$OUT/med/near.ll" --far-intrinsics
$LLC -filetype=obj "$OUT/med/near.ll" -o "$OUT/med/near.o"
"$BE/install/bin/llvm-readobj" --sections "$OUT/med/near.o" |
  awk '/Name: /{n=$2} /Size: /{if (n ~ /^\.text\./) print $2, substr(n,7)}' > "$OUT/med/sizes.txt"
python3 -I "$HERE/scripts/ir_medium.py" "$OUT/med/medium.ll" "$OUT/med/med.ll" --far-intrinsics \
  --medium "$OUT/med/sizes.txt" --cap0 0x4000 --cap 0xD000
$LLC "$OUT/med/med.ll" -o "$OUT/med/med.s"
$LLC "$BE/rt/ir/di.ll" -o "$OUT/med/rt/di.s"
python3 -I "$HERE/scripts/seg_rt.py" "$OUT/med/med.s" "$OUT/med/med2.s" "$OUT/med/rt" \
  "$BE/rt/asm/divsi.s" "$BE/rt/asm/mem.s" "$OUT/med/rt/di.s"
for f in "$OUT"/med/rt/rt_s*.s; do $MC "$f" -o "${f%.s}.o"; done
$MC "$OUT/med/med2.s" -o "$OUT/med/med2.o"
$MC "$HERE/scripts/start_far.s" -o "$OUT/med/start_far.o"
$MC "$HERE/scripts/hal_far.s" -o "$OUT/med/hal_far.o"
"$LD" -T "$HERE/scripts/med.ld" -o "$OUT/med/med.elf" -Map "$OUT/med/med.map" --exe "$OUT/medium.exe" \
  "$OUT/med/start_far.o" "$OUT/med/hal_far.o" "$OUT/med/med2.o" "$OUT"/med/rt/rt_s*.o "$BE"/rt/obj/*.o
ls -l "$OUT/tiny.com" "$OUT/medium.exe"
echo "host (tiny):   $(cat "$OUT/host_tiny.txt")"
echo "host (medium): $(cat "$OUT/host_medium.txt")"
