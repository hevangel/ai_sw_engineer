#!/usr/bin/env bash
# Every DISKCOPY check, on the host (issue hevangel/ai_sw_engineer#289):
#
#   IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/diskcopy/scripts/run_all.sh
#
# IBM_PC_ROMS: the 5150 BIOS and BASIC (system/ibm_pc/emulator/scripts/fetch_reference.py);
# XT_ROMS: the XT BIOS, BASIC and PC DOS 2.00 (../toolchain/scripts/fetch_reference.py).
# 1. host: the Rust core against the oracle transcripts (7 cases, screen and drive B; drive B
#    needs XT_ROMS) and the spec, fmt, clippy;
# 2. target: build build/diskcopy.com with the XT compiler profile;
# 3. the four-way matrix: the original and the Rust build on the Rust emulator (cargo test),
#    and the Rust build on MartyPC's 5160 (../scripts/capture_dos.sh with PROGRAM= and SAVE_B=1),
#    compared with the oracle, which is the original on MartyPC (oracle/transcripts; re-capture
#    with capture_dos.sh without PROGRAM), screen and drive B by the rule of plans/testplan.md.
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
SAVE_B=1 MARTY_CYCLES="${MARTY_CYCLES:-1500000000}" PROGRAM="$HERE/build/diskcopy.com" NAME=DISKCOPY.COM \
  OUT="$HERE/build/rust_on_martypc" WORK="$HERE/build/rust_run" bash ../scripts/capture_dos.sh "$HERE" >/dev/null
# the original's drive A of each case as capture_dos.sh builds it without PROGRAM, for the drive B
# rule (pcdisk was built by capture_dos.sh)
PCDISK="${MARTY_TARGET_DIR:-/tmp/martypc-target}/release/pcdisk"
mkdir -p build/original_a
while IFS=$'\t' read -r name files _; do
  if [[ ,$files, == *,a:system360,* ]]; then
    "$PCDISK" syscopy "$XT_ROMS/pcdos200.img" "build/original_a/$name.img" 360
  else
    cp "$XT_ROMS/pcdos200.img" "build/original_a/$name.img"
  fi
  if [[ $files != - ]]; then
    IFS=, read -ra fl <<< "$files"
    for x in "${fl[@]}"; do
      [[ $x == a:* || $x == b:* ]] || "$PCDISK" add "build/original_a/$name.img" "build/rust_run/in/$x" "$x"
    done
  fi
done < build/rust_run/cases.tsv
python3 -I scripts/compare.py oracle/transcripts build/rust_on_martypc build/rust_run build/original_a
echo "ALL DISKCOPY CHECKS PASSED"
