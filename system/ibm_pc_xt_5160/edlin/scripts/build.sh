#!/usr/bin/env bash
# Build build/edlin.com: the Rust rewrite compiled with the XT compiler profile (llvm-i8086, the
# msp430 Rust route or $I8086_RUSTC), linked at 100h with link_com.ld and the shared runtime's
# dos.s, the title's own start (src/start.s in place of crt0_com.s) and machine side
# (src/edlin.s), and wrapped by scripts/mkexe.py into the load-high .EXE that is shipped as
# EDLIN.COM (spec D1).
#
#   system/ibm_pc_xt_5160/edlin/scripts/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BE="$ROOT/llvm-i8086"
PLAT="$ROOT/system/ibm_pc/platform"
OUT="$HERE/build"
mkdir -p "$OUT/rt"
bash "$BE/scripts/build_rust.sh" "$HERE" edlin "$OUT" >"$OUT/build_rust.log" 2>&1 || { cat "$OUT/build_rust.log" >&2; exit 1; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
"$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/dos.s" -o "$OUT/rt/dos.o"
for s in start edlin; do
  "$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$HERE/src/$s.s" -o "$OUT/rt/$s.o"
done
"$LD" -T "$PLAT/link_com.ld" -o "$OUT/edlin.elf" -Map "$OUT/edlin.map" --bin "$OUT/edlin.bin" \
  "$OUT/rt/start.o" "$OUT/rt/dos.o" "$OUT/rt/edlin.o" "$OUT/edlin.o" "$BE"/rt/obj/*.o
python3 -I "$HERE/scripts/mkexe.py" "$OUT/edlin.bin" "$OUT/edlin.map" "$OUT/edlin.com"
echo "edlin.com: $(wc -c < "$OUT/edlin.com") bytes, image $(wc -c < "$OUT/edlin.bin") (the original: 4,608); sha256 $(sha256sum "$OUT/edlin.com" | cut -c1-16)..."
