#!/usr/bin/env bash
# The regression tests of the IBM PC 5150 compiler profile. Run on the host (needs docker and the
# ai-sw-engineer image, the ai_hw_engineer checkout for the firmware and the DOS disk):
#
#   system/ibm_pc/toolchain/scripts/run_tests.sh
#
# 1. build every image from source and compare their SHA-256 with report/artifacts.sha256;
# 2. independent checks: three decoders, relocations re-applied, placement, entry, file formats;
# 3. negative controls for those checks;
# 4. the images on independent models: MartyPC against the RTL's recordings;
# 5. the same images in the Rust emulator (boot_oracle, i8086_dos, i8086_rust; the stress matrix is the backend's, llvm-i8086/scripts/stress_matrix.sh).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
D() { MSYS_NO_PATHCONV=1 docker run --rm -v "$ROOT:/workspace" -v ai-sw-llvm-build:/build -w /workspace ai-sw-engineer:latest "$@"; }

echo "== 1. build"
D system/ibm_pc/toolchain/scripts/build_profile.sh >/dev/null
if diff -u "$HERE/report/artifacts.sha256" "$HERE/build/artifacts.sha256"; then echo "artifacts match report/artifacts.sha256"; else echo "ERROR: the images changed" >&2; exit 1; fi
echo "== 2. independent checks"
D bash -c "pip install --user -q capstone==5.0.7 2>/dev/null; python3 system/ibm_pc/toolchain/scripts/check_images.py system/ibm_pc/toolchain/build /workspace"
echo "== 3. negative controls"
D bash -c "pip install --user -q capstone==5.0.7 2>/dev/null; python3 system/ibm_pc/toolchain/scripts/mutants.py system/ibm_pc/toolchain/build /workspace"
echo "== 4. MartyPC against the RTL"
"$HERE/scripts/run_independent.sh"
echo "== 5. the Rust emulator"
(cd "$ROOT/system/ibm_pc/emulator" && cargo +1.99.1 test --release --test boot_oracle --test i8086_dos --test i8086_rust -- --include-ignored --skip the_stress_program 2>&1 | grep -E "^test result|FAILED")
echo "ALL PROFILE TESTS PASSED"
