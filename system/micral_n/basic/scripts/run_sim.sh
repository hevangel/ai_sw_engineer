#!/usr/bin/env bash
# Replay every oracle transcript against the target image on the
# ai_hw_engineer RTL (platform/scripts/run_tty.py compares byte for byte).
# Usage: scripts/run_sim.sh [ram|boot]   (default: both)
#   ram   the image loaded at 0x0000 and started there
#   boot  build/disk.img loaded by the Rust boot ROM (the machine's path)
# Runs in the ai-hw-engineer image (platform/sim/build_rtl.sh first).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

mode="${1:-both}"
[[ -f $IMAGE ]] || fail "no target image; run scripts/build.sh target"
run="$PLATFORM_DIR/scripts/run_tty.py"
if [[ $mode == ram || $mode == both ]]; then
    status "RTL, image in RAM"
    python3 "$run" --ram "$IMAGE" "$TITLE_DIR"/oracle/transcripts/*.txt
fi
if [[ $mode == boot || $mode == both ]]; then
    [[ -f $BOOT ]] || fail "no boot ROM; build system/$SYSTEM/boot first"
    status "RTL, booted from build/disk.img"
    python3 "$run" --disk "$DISK" --boot "$BOOT" "$TITLE_DIR"/oracle/transcripts/*.txt
fi
