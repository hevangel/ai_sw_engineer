#!/usr/bin/env bash
# Build build/diskcomp.com: the Rust rewrite compiled with the XT compiler profile (llvm-i8086, the
# msp430 Rust route or $I8086_RUSTC) and linked as a PC DOS .COM file (`i8086-ld --com`) with
# the shared DOS runtime (system/ibm_pc/platform: link_com.ld, dos.s), the title's own start
# (src/start.s in place of crt0_com.s: the stack below the buffers, spec D1) and BIOS calls
# (src/bios.s).
#
#   system/ibm_pc_xt_5160/diskcomp/scripts/build.sh
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
BE="$ROOT/llvm-i8086"
PLAT="$ROOT/system/ibm_pc/platform"
OUT="$HERE/build"
mkdir -p "$OUT/rt"
bash "$BE/scripts/build_rust.sh" "$HERE" diskcomp "$OUT" >"$OUT/build_rust.log" 2>&1 || { cat "$OUT/build_rust.log" >&2; exit 1; }
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR
"$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/dos.s" -o "$OUT/rt/dos.o"
for s in start bios; do
  "$BE/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$HERE/src/$s.s" -o "$OUT/rt/$s.o"
done
"$LD" -T "$PLAT/link_com.ld" -o "$OUT/diskcomp.elf" -Map "$OUT/diskcomp.map" --com "$OUT/diskcomp.com" \
  "$OUT/rt/start.o" "$OUT/rt/dos.o" "$OUT/rt/bios.o" "$OUT/diskcomp.o" "$BE"/rt/obj/*.o
echo "diskcomp.com: $(wc -c < "$OUT/diskcomp.com") bytes (the original: 2074; at most 5120, plans/testplan.md); sha256 $(sha256sum "$OUT/diskcomp.com" | cut -c1-16)..."
