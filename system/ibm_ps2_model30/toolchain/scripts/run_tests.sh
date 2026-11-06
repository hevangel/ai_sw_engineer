#!/usr/bin/env bash
# The regression tests of the IBM PS/2 Model 30 compiler profile (issue #60), on the host:
#
#   system/ibm_ps2_model30/toolchain/scripts/run_tests.sh
#
# Needs llvm-i8086/install/bin, cargo 1.99.1, the pinned nightly with rust-src, python3 with
# capstone 5.0.7 and GNU binutils.
# 1. build every image from source; their SHA-256 must equal report/artifacts.sha256;
# 2. independent checks (three decoders, relocations, placement, entry, formats) and their negative
#    controls (the 5150 profile's scripts).
# The execution gate (the images on an emulator of the Model 30) is not automated yet: see
# report/report.md.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
P5150="$ROOT/system/ibm_pc/toolchain/scripts"
echo "== 1. build"
bash "$HERE/scripts/build_profile.sh" >/dev/null
diff -u "$HERE/report/artifacts.sha256" "$HERE/build/artifacts.sha256" && echo "artifacts match report/artifacts.sha256"
echo "== 2. independent checks and negative controls"
python3 "$P5150/check_images.py" "$HERE/build" "$ROOT" | tail -1
python3 "$P5150/mutants.py" "$HERE/build" "$ROOT" | tail -1
echo "BUILD AND CHECKS PASSED (execution not run, see report/report.md)"
