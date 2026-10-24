#!/usr/bin/env bash
# Shared settings for basic scripts. Sourced, not executed.

SYSTEM="micral_n"
SOFTWARE="basic"

TITLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$TITLE_DIR/../../.." && pwd)"
PLATFORM_DIR="$ROOT/system/$SYSTEM/platform"
# shellcheck source=/dev/null
source "$PLATFORM_DIR/target.env" # sets CHIP
BACKEND_DIR="$ROOT/llvm-$CHIP"
BUILD_DIR="$TITLE_DIR/build"
AI_HW_ENGINEER="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"

# ROM budget in bytes for this title (from platform/memory_map.md); 0 = unknown.
ROM_BUDGET="${ROM_BUDGET:-0}"

status() { printf '==> %s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
