#!/usr/bin/env bash
# Execute the llvm-mos6502 smoke image on the mre/mos6502 emulator
# (external/mos6502, BSD-3-Clause; run, never copied) via tools/mos6502_run,
# and compare the results window with the independent expected values from
# scripts/expected.py (the source-semantics oracle).
#
# Runs INSIDE the ai-sw-engineer container, from the repository root, after
# scripts/build.sh:
#
#   llvm-mos6502/scripts/exec_mos6502.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # llvm-mos6502/
runner="$here/build/mos6502-run"
image="$here/build/smoke.bin"

if [[ ! -x $runner ]]; then
    echo "==> building tools/mos6502_run (host cargo)"
    cargo build --release --manifest-path "$here/tools/mos6502_run/Cargo.toml"
    # The image sets CARGO_TARGET_DIR=/workspace/target; honor it, else the
    # crate-local target dir.
    src="${CARGO_TARGET_DIR:-$here/tools/mos6502_run/target}/release/mos6502-run"
    cp "$src" "$runner"
fi
[[ -f $image ]] || { echo "ERROR: $image missing (run scripts/build.sh)" >&2; exit 1; }

"$runner" "$image" > "$here/build/window_mos6502.txt"
py "$here/scripts/expected.py" > "$here/build/window_expected.txt" 2>/dev/null \
  || python3 "$here/scripts/expected.py" > "$here/build/window_expected.txt"

if diff "$here/build/window_expected.txt" "$here/build/window_mos6502.txt" > /dev/null; then
    echo "PASS: mre/mos6502 window matches the independent expected values"
else
    echo "FAIL: window differs (expected vs mre/mos6502):" >&2
    diff "$here/build/window_expected.txt" "$here/build/window_mos6502.txt" >&2 || true
    exit 1
fi
