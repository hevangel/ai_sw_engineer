#!/usr/bin/env bash
# The regression tests of the IBM PC XT 5160 compiler profile (issue #59), on the host:
#
#   XT_ROMS=DIR system/ibm_pc_xt_5160/toolchain/scripts/run_tests.sh
#
# Needs llvm-i8086/install/bin (llvm-i8086/scripts/build_llvm.sh), cargo 1.99.1, the pinned nightly
# with rust-src, python3 with capstone 5.0.7, GNU binutils, and XT_ROMS (scripts/fetch_reference.py).
# 1. the profile's own consistency (tests/test_profile.py);
# 2. build every image from source; their SHA-256 must equal report/artifacts.sha256;
# 3. independent checks (three decoders, relocations, placement, entry, formats) and their negative
#    controls: the 5150 profile's scripts, unchanged;
# 4. the images on MartyPC's 5160 against expectations that do not come from MartyPC, and a negative
#    control of that comparison.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
P5150="$ROOT/system/ibm_pc/toolchain/scripts"

echo "== 1. profile consistency"
python3 -I "$HERE/../tests/test_profile.py"
echo "== 2. build"
bash "$HERE/scripts/build_profile.sh" >/dev/null
diff -u "$HERE/report/artifacts.sha256" "$HERE/build/artifacts.sha256" && echo "artifacts match report/artifacts.sha256"
echo "== 3. independent checks and negative controls"
python3 "$P5150/check_images.py" "$HERE/build" "$ROOT" | tail -1
python3 "$P5150/mutants.py" "$HERE/build" "$ROOT" | tail -1
echo "== 4. MartyPC 5160"
bash "$HERE/scripts/run_independent.sh"
# negative control: a wrong expectation must fail the comparison
sed 's/^ram = 256 KiB/ram = 128 KiB/' "$HERE/expected/xt_info.txt" > "$HERE/build/wrong.txt"
if python3 -I "$HERE/scripts/compare_screen.py" "$HERE/build/screens/xt_info.txt" "$HERE/build/wrong.txt" >/dev/null; then
  echo "ERROR: a wrong expectation was not detected" >&2; exit 1
fi
echo "negative control: a wrong expected RAM size is reported"
echo "ALL XT PROFILE TESTS PASSED"
