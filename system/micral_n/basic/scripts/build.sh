#!/usr/bin/env bash
# Build SCELBAL: host check, then the target image through llvm-i8008.
# Usage: scripts/build.sh [host|target|all]   (default: all)
# Runs in the ai-sw-engineer image (pinned nightly, llvm-i8008/install).
# target: build/basic.o (build_rust.sh), build/basic.bin + .map
# (build_image.sh), build/disk.img (the image as the boot ROM loads it).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

stage="${1:-all}"
mkdir -p "$BUILD_DIR"
cd "$TITLE_DIR"

if [[ $stage == host || $stage == all ]]; then
    status "host build"
    cargo build --release
fi

if [[ $stage == target || $stage == all ]]; then
    status "target image via llvm-$CHIP"
    bash "$BACKEND_DIR/scripts/build_rust.sh" "$TITLE_DIR" "$SOFTWARE" "$BUILD_DIR"
    bash "$BACKEND_DIR/scripts/build_image.sh" "$LAYOUT" "$IMAGE" "${ASM_SRCS[@]}" "$BUILD_DIR/$SOFTWARE.o"
    python3 "$PLATFORM_DIR/scripts/mkdisk.py" "$IMAGE" "$DISK"
    size=$(wc -c < "$IMAGE" | tr -d ' ')
    status "image size: $size / $ROM_BUDGET bytes"
    [[ $size -le $ROM_BUDGET ]] || fail "image exceeds the boot ROM's load area by $((size - ROM_BUDGET)) bytes"
fi
