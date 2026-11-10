#!/usr/bin/env bash
# Build build/basic.com: the Rust rewrite as a medium-model PC DOS .EXE named BASIC.COM (spec D1),
# with llvm-i8086's medium-model route (llvm-i8086/spec/abi.md D10): build_rust_medium.sh packs
# the functions into code segments and gives each its runtime copies; rt/medium/start_far.s
# and far.s are the start and the BIOS/DOS calls.
#
#   system/ibm_pc_xt_5160/basic/scripts/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BE="$ROOT/llvm-i8086"
OUT="$HERE/build"
mkdir -p "$OUT/obj"
MEDIUM_CAP0="${MEDIUM_CAP0:-0x1000}" MEDIUM_CAP="${MEDIUM_CAP:-0xE000}" \
  bash "$BE/scripts/build_rust_medium.sh" "$HERE" basic "$OUT/obj" >"$OUT/build_rust.log" 2>&1 ||
  { cat "$OUT/build_rust.log" >&2; exit 1; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
for s in start_far far; do
  "$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$BE/rt/medium/$s.s" -o "$OUT/obj/$s.o"
done
shopt -s nullglob
"$LD" -T "$OUT/obj/basic.ld" -o "$OUT/basic.elf" -Map "$OUT/basic.map" --exe "$OUT/basic.com" \
  "$OUT/obj/start_far.o" "$OUT/obj/far.o" "$OUT/obj/basic_medium.o" "$OUT"/obj/rt_s*.o "$BE"/rt/obj/*.o \
  | tee "$OUT/link.log"
echo "basic.com: $(wc -c < "$OUT/basic.com") bytes; sha256 $(sha256sum "$OUT/basic.com" | cut -c1-16)..."
