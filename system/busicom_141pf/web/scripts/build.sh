#!/usr/bin/env bash
# Build the Busicom 141-PF browser machine WASM with the pinned toolchain and
# copy the artifact next to the UI. Run from the repository root.
#
#   system/busicom_141pf/web/scripts/build.sh
#
# Output: system/busicom_141pf/web/www/busicom_emulator_wasm.wasm (gitignored).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
web="$(cd "$here/.." && pwd)"
root="$(cd "$web/../../.." && pwd)"

TOOLCHAIN="${RUST_TOOLCHAIN:-1.99.1}"   # the repo's pinned rustc (see Dockerfile)
TARGET=wasm32-unknown-unknown
out="$web/wasm/target/$TARGET/release/busicom_emulator_wasm.wasm"

command -v cargo >/dev/null || { echo "ERROR: cargo not on PATH" >&2; exit 1; }

cd "$web/wasm"
cargo "+$TOOLCHAIN" build --release --target "$TARGET"
cp "$out" "$web/www/busicom_emulator_wasm.wasm"

echo "==> artifact: www/busicom_emulator_wasm.wasm"
wc -c "$web/www/busicom_emulator_wasm.wasm"
sha256sum "$web/www/busicom_emulator_wasm.wasm"
