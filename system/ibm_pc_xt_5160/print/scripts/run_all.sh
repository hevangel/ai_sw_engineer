#!/usr/bin/env bash
# Every PRINT check, on the host (issue hevangel/ai_sw_engineer#297):
#
#   IBM_PC_ROMS=DIR XT_ROMS=DIR XT86_ROMS=DIR BOX86=FILE system/ibm_pc_xt_5160/print/scripts/run_all.sh
#
# IBM_PC_ROMS: the 5150 BIOS and BASIC (system/ibm_pc/emulator/scripts/fetch_reference.py);
# XT_ROMS: the XT BIOS, BASIC and PC DOS 2.00 (../toolchain/scripts/fetch_reference.py);
# XT86_ROMS: the two 5160 ROM chips for 86Box; BOX86: an 86Box built with -D AUTOMATION=ON.
# 1. host: the Rust core and the resident model against the oracle transcripts (10 cases, the
#    screen and the printed lines) and the spec, fmt, clippy;
# 2. target: build build/print.com with the XT compiler profile;
# 3. the four-way matrix: the original and the Rust build on the Rust emulator (cargo test,
#    with its printer), and the Rust build on 86Box's XT with a text printer on LPT1
#    (../scripts/capture_dos.sh with ORACLE=86box and PROGRAM=), compared with the oracle,
#    which is the original on 86Box (oracle/transcripts; re-capture with capture_dos.sh
#    without PROGRAM).
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
echo "== 3b. Rust build on 86Box's XT"
ORACLE=86box PROGRAM="$HERE/build/print.com" NAME=PRINT.COM OUT="$HERE/build/rust_on_86box" WORK="$HERE/build/rust_run" \
  bash ../scripts/capture_dos.sh "$HERE" >/dev/null
python3 -I scripts/compare.py oracle/transcripts build/rust_on_86box
echo "ALL PRINT CHECKS PASSED"
