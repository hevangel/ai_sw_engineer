#!/usr/bin/env bash
# Shared settings for the shooting_stars scripts. Sourced, not executed.

SYSTEM="micral_n"
SOFTWARE="shooting_stars"

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
ASM_SRCS=("$TITLE_DIR/asm/start.s" "$PLATFORM_DIR/runtime/tty.s")
IMAGE="$BUILD_DIR/$SOFTWARE.bin"
DISK="$BUILD_DIR/disk.img"
BOOT="$ROOT/system/$SYSTEM/boot/build/boot.bin"

# What the boot ROM loads (platform/memory_map.md): the image ends below 0x3700.
ROM_BUDGET=$((0x3700))

status() { printf '==> %s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

# Extra llc options (spec/codegen.md of llvm-i8008, sec. 5).
export LLC_FLAGS=""
