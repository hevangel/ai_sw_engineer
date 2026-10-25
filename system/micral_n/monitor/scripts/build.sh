#!/usr/bin/env bash
# Build monitor: host check, then the target image through llvm-i8008.
# Usage: scripts/build.sh [host|target|all]   (default: all)
# Runs in the ai-sw-engineer image (pinned nightly, llvm-i8008/install).
# target: build/monitor.o (build_rust.sh), build/monitor.bin + .map
# (build_image.sh).
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
    # The test image: the same code in 8 KiB at 0x2000, which the RTL replay
    # uses while the ROM image does not fit (report/final_report.md).
    bash "$BACKEND_DIR/scripts/build_image.sh" "$TITLE_DIR/layout_test.ld" "$BUILD_DIR/monitor_test.bin"         "${ASM_SRCS[@]}" "$BUILD_DIR/$SOFTWARE.o"
    if bash "$BACKEND_DIR/scripts/build_image.sh" "$LAYOUT" "$IMAGE" "${ASM_SRCS[@]}" "$BUILD_DIR/$SOFTWARE.o"; then
        status "ROM image: $(wc -c < "$IMAGE" | tr -d ' ') / $ROM_BUDGET bytes"
    else
        status "ROM image does not fit $ROM_BUDGET bytes (known gap)"
        [[ -z ${STRICT:-} ]] || fail "STRICT: the ROM image must fit"
    fi
fi
