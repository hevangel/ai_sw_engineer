#!/usr/bin/env bash
# Run the target image against every oracle transcript on a simulator.
# SIM=primary (ai_hw_engineer, default) | secondary (external emulator)
# Output: build/sim/<transcript>.txt in transcript format (only '<' lines matter).
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

SIM="${SIM:-primary}"
image="$BUILD_DIR/target/$SOFTWARE.bin"
runner="$PLATFORM_DIR/scripts/run_image.sh"

[[ -f $image ]] || fail "no target image; run scripts/build.sh target"
[[ -x $runner ]] || fail "$runner missing: the busicom_141pf platform has no simulator runner yet"

mkdir -p "$BUILD_DIR/sim"
for t in "$TITLE_DIR"/oracle/transcripts/*.txt; do
    name=$(basename "$t")
    status "$SIM: $name"
    # Contract: run_image.sh <sim> <image> <input_transcript> > <output_transcript>
    "$runner" "$SIM" "$image" "$t" > "$BUILD_DIR/sim/$name"
done
