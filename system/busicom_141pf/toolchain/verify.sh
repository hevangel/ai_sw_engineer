#!/usr/bin/env bash
# End-to-end verification of the Busicom 141-PF compiler profile (issue #53).
#
# Runs INSIDE the ai-sw-engineer container, from the repository root:
#
#   docker run --rm -v <repo>:/workspace -v ai-sw-llvm-build:/llvm-build \
#     -w /workspace ai-sw-engineer:latest bash -lc \
#     'system/busicom_141pf/toolchain/verify.sh'
#
# Gates: whole-program IR -> target ROM image (size gate inside the packer)
# -> sha256 -> 42-example replay on the independent external/chips board model
# (xl board) -> the printed-tape payload must equal the committed original-ROM
# oracle trace. The primary RTL executor (run_image.sh primary, ai-hw-engineer
# Verilator image) is documented in README.md and is not run here.
set -euo pipefail

root="$(pwd)"
title="system/busicom_141pf/firmware"
out="$title/build/target"
trace="$title/build/trace_verify_chips.txt"
oracle="$title/oracle/rom_port_trace.txt"

status() { printf '==> %s\n' "$*"; }

status "1/5 whole-program IR (pinned nightly, avr-none -> i4004 retarget)"
"$title/scripts/build_target_ir.sh" > /dev/null

status "2/5 target image (llc + llvm-mc + i4004_pack, xl linker script)"
llvm-i4004/scripts/build_image.sh "$title" "$out" \
    "$root/system/busicom_141pf/platform/link_xl.ld"

image="$out/firmware.bin"
status "3/5 image: $(wc -c < "$image") bytes (budget 4096), sha256:"
sha256sum "$image"

status "4/5 replay 42 manual examples on external/chips (xl board)"
system/busicom_141pf/platform/scripts/run_image.sh secondary \
    "$image" "$trace" --board xl

status "5/5 compare printed-tape payload with the original-ROM oracle"
if diff <(grep -v '^#' "$trace") <(grep -v '^#' "$oracle") > /dev/null; then
    status "PASS: compiled firmware tape identical to the original-ROM oracle"
else
    echo "ERROR: compiled firmware tape differs from the oracle:" >&2
    diff <(grep -v '^#' "$trace") <(grep -v '^#' "$oracle") | head -20 >&2
    exit 1
fi
