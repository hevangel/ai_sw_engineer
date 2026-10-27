#!/usr/bin/env bash
# Execute the llvm-m68k smoke image on the independent r68k emulator
# (Musashi-ported, MIT) and compare the results window with the independent
# expected values from scripts/expected.py.
#
# Runs INSIDE the ai-sw-engineer container, from the repository root, after
# scripts/build.sh:
#
#   llvm-m68k/scripts/exec_r68k.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # llvm-m68k/
runner="$here/build/r68k-run"
image="$here/build/smoke.bin"

if [[ ! -x $runner ]]; then
    echo "==> building tools/r68k_run (host cargo)"
    cargo build --release --manifest-path "$here/tools/r68k_run/Cargo.toml"
    src="${CARGO_TARGET_DIR:-$here/tools/r68k_run/target}/release/r68k-run"
    cp "$src" "$runner"
fi
[[ -f $image ]] || { echo "ERROR: $image missing (run scripts/build.sh)" >&2; exit 1; }

"$runner" "$image" > "$here/build/window_r68k.txt"
py "$here/scripts/expected.py" > "$here/build/window_expected.txt" 2>/dev/null \
  || python3 "$here/scripts/expected.py" > "$here/build/window_expected.txt"

if diff "$here/build/window_expected.txt" "$here/build/window_r68k.txt" > /dev/null; then
    echo "PASS: r68k window matches the independent expected values"
else
    echo "FAIL: window differs (expected vs r68k):" >&2
    diff "$here/build/window_expected.txt" "$here/build/window_r68k.txt" >&2 || true
    exit 1
fi
