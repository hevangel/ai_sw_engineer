#!/usr/bin/env bash
# Replay the monitor transcripts on the RTL (scripts/run_target.py) with the
# ROM image build/monitor.bin at 0x3800.
# Runs in the ai-hw-engineer image (platform/sim/build_rtl.sh first).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

img="$IMAGE"
[[ -f $img ]] || fail "no ROM image; run scripts/build.sh target"
status "RTL: monitor ROM"
python3 "$TITLE_DIR/scripts/run_target.py" "$img" "${img%.bin}.map"
