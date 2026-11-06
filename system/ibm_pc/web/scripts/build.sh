#!/usr/bin/env bash
# Build the IBM PC 5150 browser machine (the native emulator crate compiled to
# WebAssembly) with the pinned toolchain and copy it next to the UI.
# Run from anywhere:
#
#   system/ibm_pc/web/scripts/build.sh
#
# Output: system/ibm_pc/web/www/ibm_pc_emulator_wasm.wasm (gitignored; rebuild
# after any emulator change).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
web="$(cd "$here/.." && pwd)"

TOOLCHAIN="${RUST_TOOLCHAIN:-1.99.1}"   # the repo's pinned rustc (see Dockerfile)
TARGET=wasm32-unknown-unknown
out="$web/wasm/target/$TARGET/release/ibm_pc_emulator_wasm.wasm"

command -v cargo >/dev/null || { echo "ERROR: cargo not on PATH" >&2; exit 1; }
rustup target list --toolchain "$TOOLCHAIN" --installed 2>/dev/null | grep -qx "$TARGET" || {
  echo "ERROR: rustup target add --toolchain $TOOLCHAIN $TARGET" >&2; exit 1; }

cd "$web/wasm"
cargo "+$TOOLCHAIN" build --release --target "$TARGET"
cp "$out" "$web/www/ibm_pc_emulator_wasm.wasm"

echo "==> artifact: www/ibm_pc_emulator_wasm.wasm"
wc -c "$web/www/ibm_pc_emulator_wasm.wasm"
sha256sum "$web/www/ibm_pc_emulator_wasm.wasm"
