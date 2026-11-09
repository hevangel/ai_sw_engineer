#!/usr/bin/env bash
# Build build/debug.com: the Rust rewrite compiled with the XT compiler profile (llvm-i8086, the
# msp430 Rust route or $I8086_RUSTC), linked at 100h with link_com.ld and the shared runtime's
# dos.s, the title's own start (src/start.s in place of crt0_com.s) and machine side
# (src/debug.s), and wrapped by scripts/mkexe.py into the load-high .EXE that is shipped as
# DEBUG.COM (spec D1).
#
#   system/ibm_pc_xt_5160/debug/scripts/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BE="$ROOT/llvm-i8086"
PLAT="$ROOT/system/ibm_pc/platform"
OUT="$HERE/build"
mkdir -p "$OUT/rt"
bash "$BE/scripts/build_rust.sh" "$HERE" debug "$OUT" >"$OUT/build_rust.log" 2>&1 || { cat "$OUT/build_rust.log" >&2; exit 1; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
"$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/dos.s" -o "$OUT/rt/dos.o"
for s in start debug; do
  "$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$HERE/src/$s.s" -o "$OUT/rt/$s.o"
done
"$LD" -T "$PLAT/link_com.ld" -o "$OUT/debug.elf" -Map "$OUT/debug.map" --bin "$OUT/debug.bin" \
  "$OUT/rt/start.o" "$OUT/rt/dos.o" "$OUT/rt/debug.o" "$OUT/debug.o" "$BE"/rt/obj/*.o
python3 -I "$HERE/scripts/mkexe.py" "$OUT/debug.bin" "$OUT/debug.map" "$OUT/debug.com"
echo "debug.com: $(wc -c < "$OUT/debug.com") bytes, image $(wc -c < "$OUT/debug.bin") (the original: 11,904); sha256 $(sha256sum "$OUT/debug.com" | cut -c1-16)..."
