#!/usr/bin/env bash
# Build build/assign.com: the Rust rewrite compiled with the XT compiler profile (llvm-i8086, the
# msp430 Rust route or $I8086_RUSTC) and linked as a PC DOS .COM (a flat image loaded at 100h,
# `i8086-ld --bin`) with link_com.ld and the shared runtime's dos.s; src/resident.s takes the
# place of crt0_com.s: it is linked first and holds the resident data and hooks at 100h-302h
# and the start at 303h (spec D1-D3).
#
#   system/ibm_pc_xt_5160/assign/scripts/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BE="$ROOT/llvm-i8086"
PLAT="$ROOT/system/ibm_pc/platform"
OUT="$HERE/build"
mkdir -p "$OUT/rt"
bash "$BE/scripts/build_rust.sh" "$HERE" assign "$OUT" >"$OUT/build_rust.log" 2>&1 || { cat "$OUT/build_rust.log" >&2; exit 1; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
"$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/dos.s" -o "$OUT/rt/dos.o"
"$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$HERE/src/resident.s" -o "$OUT/rt/resident.o"
"$LD" -T "$PLAT/link_com.ld" -o "$OUT/assign.elf" -Map "$OUT/assign.map" --bin "$OUT/assign.com" \
  "$OUT/rt/resident.o" "$OUT/rt/dos.o" "$OUT/assign.o" "$BE"/rt/obj/*.o
echo "assign.com: $(wc -c < "$OUT/assign.com") bytes (the original: 896; resident 100h-30Fh); sha256 $(sha256sum "$OUT/assign.com" | cut -c1-16)..."
