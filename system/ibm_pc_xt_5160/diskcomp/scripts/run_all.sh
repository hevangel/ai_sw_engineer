#!/usr/bin/env bash
# Every DISKCOMP check, on the host (issue hevangel/ai_sw_engineer#291):
#
#   IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/diskcomp/scripts/run_all.sh
#
# IBM_PC_ROMS: the 5150 BIOS and BASIC (system/ibm_pc/emulator/scripts/fetch_reference.py);
# XT_ROMS: the XT BIOS, BASIC and PC DOS 2.00 (../toolchain/scripts/fetch_reference.py).
# 1. target: build build/diskcomp.com with the XT compiler profile;
# 2. host: the Rust core against the oracle transcripts (8 cases; they need XT_ROMS), the tests
#    from the spec, the check that the rewrite on the disks changes no reported track (needs
#    build/diskcomp.com), fmt, clippy;
# 3. the four-way matrix: the original and the Rust build on the Rust emulator (cargo test),
#    and the Rust build on MartyPC's 5160 (../scripts/capture_dos.sh with PROGRAM=), compared
#    with the oracle, which is the original on MartyPC (oracle/transcripts; re-capture with
#    capture_dos.sh without PROGRAM), by the screen rule of plans/testplan.md.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$HERE"
echo "== 1. target"
bash scripts/build.sh
echo "== 2. host"
cargo +1.99.1 test --quiet 2>&1 | grep "test result"
cargo +1.99.1 fmt -- --check
cargo +1.99.1 clippy --quiet --all-targets -- -D warnings
echo "== 3a. original and Rust build on the Rust emulator"
cargo +1.99.1 test --release --quiet --test rust_emulator -- --ignored 2>&1 | grep "test result"
echo "== 3b. Rust build on MartyPC 5160"
MARTY_CYCLES="${MARTY_CYCLES:-1500000000}" PROGRAM="$HERE/build/diskcomp.com" NAME=DISKCOMP.COM \
  OUT="$HERE/build/rust_on_martypc" WORK="$HERE/build/rust_run" bash ../scripts/capture_dos.sh "$HERE" >/dev/null
python3 -I scripts/compare.py oracle/transcripts build/rust_on_martypc
echo "ALL DISKCOMP CHECKS PASSED"
