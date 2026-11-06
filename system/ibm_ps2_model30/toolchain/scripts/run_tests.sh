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
# 3. with PS2_ROMS and BOX86 set: the images on 86Box's PS/2 Model 30 against expectations that do not
#    come from 86Box (scripts/run_independent.sh), and a negative control of that comparison.
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
if [[ -n ${PS2_ROMS:-} && -n ${BOX86:-} ]]; then
  echo "== 3. 86Box PS/2 Model 30"
  bash "$HERE/scripts/run_independent.sh"
  sed 's/^model byte = 0xfa/model byte = 0xfc/' "$HERE/expected/ps2_info.txt" > "$HERE/build/wrong.txt"
  if python3 -I "$ROOT/system/ibm_pc_xt_5160/toolchain/scripts/compare_screen.py" "$HERE/build/screens/ps2_info.txt" "$HERE/build/wrong.txt" >/dev/null; then
    echo "ERROR: a wrong expectation was not detected" >&2; exit 1
  fi
  echo "negative control: a wrong expected model byte is reported"
  echo "ALL PS/2 PROFILE TESTS PASSED"
else
  echo "BUILD AND CHECKS PASSED (set PS2_ROMS and BOX86 to run the images on 86Box)"
fi
