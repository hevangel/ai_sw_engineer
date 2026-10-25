#!/usr/bin/env bash
# Shared settings for the boot scripts. Sourced, not executed.

SYSTEM="micral_n"
SOFTWARE="boot"

TITLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$TITLE_DIR/../../.." && pwd)"
PLATFORM_DIR="$ROOT/system/$SYSTEM/platform"
# shellcheck source=/dev/null
source "$PLATFORM_DIR/target.env" # sets CHIP
BACKEND_DIR="$ROOT/llvm-$CHIP"
BUILD_DIR="$TITLE_DIR/build"
AI_HW_ENGINEER="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"

# The image: asm sources linked with the Rust object, and the layout.
LAYOUT="$TITLE_DIR/layout.ld"
ASM_SRCS=("$TITLE_DIR/asm/io.s")
IMAGE="$BUILD_DIR/$SOFTWARE.bin"
DISK="$BUILD_DIR/disk.img"
BOOT="$ROOT/system/$SYSTEM/boot/build/boot.bin"

# The 256-byte boot EPROM at 0x0000 (platform/memory_map.md).
ROM_BUDGET=$((0x100))

status() { printf '==> %s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

# Extra llc options (spec/codegen.md of llvm-i8008, sec. 5).
export LLC_FLAGS="-i8008-data-page"
