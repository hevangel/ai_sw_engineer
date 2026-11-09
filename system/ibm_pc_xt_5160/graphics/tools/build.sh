#!/usr/bin/env bash
# Build the capture tools (README.md) into graphics/build/tools/: PRNCAP.COM, PRNDUMP.COM and
# DRAW.COM, each one assembly file assembled with llvm-i8086's llvm-mc and linked as a PC DOS
# .COM (loaded at 100h) with the platform's link_com.ld by i8086-ld --bin.
#
#   system/ibm_pc_xt_5160/graphics/tools/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
BE="$ROOT/llvm-i8086"
OUT="$HERE/../build/tools"
mkdir -p "$OUT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
for t in prncap prndump draw; do
  "$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$HERE/$t.s" -o "$OUT/$t.o"
  "$LD" -T "$ROOT/system/ibm_pc/platform/link_com.ld" -o "$OUT/$t.elf" --bin "$OUT/${t^^}.COM" "$OUT/$t.o" 2>/dev/null
  echo "${t^^}.COM: $(wc -c < "$OUT/${t^^}.COM") bytes, sha256 $(sha256sum "$OUT/${t^^}.COM" | cut -c1-16)..."
done
