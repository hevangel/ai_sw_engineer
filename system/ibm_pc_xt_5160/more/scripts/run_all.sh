#!/usr/bin/env bash
# Every MORE check, on the host (issue hevangel/ai_sw_engineer#286):
#
#   IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/more/scripts/run_all.sh
#
# IBM_PC_ROMS: the 5150 BIOS and BASIC (system/ibm_pc/emulator/scripts/fetch_reference.py);
# XT_ROMS: the XT BIOS, BASIC and PC DOS 2.00 (../toolchain/scripts/fetch_reference.py).
# 1. host: the Rust core against the oracle transcripts (10 cases) and the rule tests, fmt, clippy;
# 2. target: build build/more.com with the XT compiler profile;
# 3. the four-way matrix: the original and the Rust build on the Rust emulator (cargo test),
#    and the Rust build on MartyPC's 5160 (../scripts/capture_dos.sh with PROGRAM=), compared
#    with the oracle, which is the original on MartyPC (oracle/transcripts).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$HERE"
echo "== 1. host"
cargo +1.99.1 test --quiet 2>&1 | grep "test result"
cargo +1.99.1 fmt -- --check
cargo +1.99.1 clippy --quiet --all-targets -- -D warnings
echo "== 2. target"
bash scripts/build.sh
echo "== 3a. original and Rust build on the Rust emulator"
cargo +1.99.1 test --release --quiet --test rust_emulator -- --ignored 2>&1 | grep "test result"
echo "== 3b. Rust build on MartyPC 5160"
PROGRAM="$HERE/build/more.com" NAME=MORE.COM OUT="$HERE/build/rust_on_martypc" WORK="$HERE/build/rust_run" \
  bash "$HERE/../scripts/capture_dos.sh" "$HERE" >/dev/null
python3 -I scripts/compare.py oracle/transcripts build/rust_on_martypc
echo "ALL MORE CHECKS PASSED"
