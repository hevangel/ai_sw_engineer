#!/usr/bin/env bash
# Build build/more.com: the Rust rewrite compiled with the XT compiler profile (llvm-i8086, the
# msp430 Rust route or $I8086_RUSTC) and linked as a PC DOS .COM file (`i8086-ld --com`) with
# the shared DOS runtime (system/ibm_pc/platform: link_com.ld, crt0_com.s, dos.s) and the
# title's own INT 10h call (src/video.s).
#
#   system/ibm_pc_xt_5160/more/scripts/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BE="$ROOT/llvm-i8086"
PLAT="$ROOT/system/ibm_pc/platform"
OUT="$HERE/build"
mkdir -p "$OUT/rt"
bash "$BE/scripts/build_rust.sh" "$HERE" more "$OUT" >"$OUT/build_rust.log" 2>&1 || { cat "$OUT/build_rust.log" >&2; exit 1; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
for s in crt0_com dos; do
  "$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/$s.s" -o "$OUT/rt/$s.o"
done
"$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$HERE/src/video.s" -o "$OUT/rt/video.o"
"$LD" -T "$PLAT/link_com.ld" -o "$OUT/more.elf" -Map "$OUT/more.map" --com "$OUT/more.com" \
  "$OUT/rt/crt0_com.o" "$OUT/rt/dos.o" "$OUT/rt/video.o" "$OUT/more.o" "$BE"/rt/obj/*.o
echo "more.com: $(wc -c < "$OUT/more.com") bytes (the original: 384); sha256 $(sha256sum "$OUT/more.com" | cut -c1-16)..."
