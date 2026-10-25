#!/usr/bin/env bash
# Replay the monitor transcripts on the RTL (scripts/run_target.py), with
# the test image build/monitor_test.bin (layout_test.ld).
# Runs in the ai-hw-engineer image (platform/sim/build_rtl.sh first).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

img="$BUILD_DIR/monitor_test.bin"
[[ -f $img ]] || fail "no test image; run scripts/build.sh target"
status "RTL: monitor test image"
python3 "$TITLE_DIR/scripts/run_target.py" "$img" "${img%.bin}.map"
