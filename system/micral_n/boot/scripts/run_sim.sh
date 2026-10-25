#!/usr/bin/env bash
# The boot ROM's target test: boot the other titles' disk images on the RTL
# and replay their transcripts (their scripts/build.sh target first).
# Runs in the ai-hw-engineer image (platform/sim/build_rtl.sh first).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

[[ -f $IMAGE ]] || fail "no boot ROM; run scripts/build.sh target"
for t in shooting_stars basic; do
    disk="$ROOT/system/$SYSTEM/$t/build/disk.img"
    [[ -f $disk ]] || fail "no $disk; build system/$SYSTEM/$t first"
    status "RTL: boot $t"
    python3 "$PLATFORM_DIR/scripts/run_tty.py" --disk "$disk" --boot "$IMAGE" \
        "$ROOT/system/$SYSTEM/$t"/oracle/transcripts/*.txt
done
