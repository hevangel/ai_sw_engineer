#!/usr/bin/env bash
# Build build/target/chkdsk.com: the Rust core compiled by rustc (msp430 route) and
# llvm-i8086, linked with the DOS .COM runtime. Run in the ai-sw-engineer image.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
OUT="$BUILD_DIR/target"
mkdir -p "$OUT"
status "chkdsk -> i8086 object"
"$BACKEND_DIR/scripts/build_rust.sh" "$TITLE_DIR" chkdsk "$OUT" >&2
status "link chkdsk.com"
LD="${I8086_LD:-}"
if [[ -z $LD ]]; then
    CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}" cargo build --quiet --release --manifest-path "$BACKEND_DIR/asm/Cargo.toml"
    LD="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}/release/i8086-ld"
fi
RT="$PLATFORM_DIR/runtime"
"$LD" -T "$PLATFORM_DIR/link_com.ld" --bin "$OUT/chkdsk.com" -Map "$OUT/chkdsk.map" \
    "$RT/crt0_com.o" "$RT/dos.o" "$OUT/chkdsk.o" "$BACKEND_DIR/rt/librt.a"
size=$(wc -c < "$OUT/chkdsk.com" | tr -d ' ')
status "chkdsk.com: $size bytes (the original: 1395)"
