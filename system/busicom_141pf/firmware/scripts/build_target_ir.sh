#!/usr/bin/env bash
# Build the whole-program, optimised i4004-compatible firmware IR.
#
#   Rust (firmware crate + core via build-std)
#     --(pinned nightly rustc, target avr-none, fat LTO, opt-level=z)-->
#   one whole-program avr-none LLVM-IR module
#     --(llvm-i4004/rust/retarget_ir.py: triple+datalayout -> i4004)-->
#   build/firmware_i4004.ll
#
# WHY avr-none: the pinned rustc's LLVM has no i4004 target, so rustc cannot
# emit i4004 IR directly. avr-none is the closest EXISTING target whose data
# layout is compatible (little-endian, 16-bit pointers at 8-bit alignment, n8;
# see llvm-i4004/rust/README.md §Mapping). Fat LTO + build-std pull the firmware
# crate and the needed bits of `core` into a single module; panic=abort and
# opt-level=z keep panic/formatting machinery out of it.
#
# Run inside the ai-sw-engineer container:
#   docker run --rm -v <fw-worktree>:/workspace ai-sw-engineer:latest \
#     bash -lc 'system/busicom_141pf/firmware/scripts/build_target_ir.sh'
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
title_dir="$(cd "$here/.." && pwd)"
root="$(cd "$title_dir/../../.." && pwd)"
build_dir="$title_dir/build"
retarget="$root/llvm-i4004/rust/retarget_ir.py"

NIGHTLY="${RUST_NIGHTLY:-nightly-2026-10-15}"
AVR_CPU="${AVR_CPU:-atmega328p}"   # any n8/16-bit-pointer avr2 cpu; code is CPU-agnostic here

mkdir -p "$build_dir/target"
cd "$title_dir"

status() { printf '==> %s\n' "$*"; }

status "emit whole-program avr-none IR (fat LTO, opt-level=z, panic=abort)"
# A stub linker lets rustc finish codegen (which emits the IR) without a real
# avr-gcc; we only want the .ll, never a linked AVR binary.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/avr-ir-build}"
export RUSTFLAGS="-C target-cpu=$AVR_CPU -C opt-level=z -C panic=abort -C lto=fat -C embed-bitcode=yes -C linker=/bin/true"
cargo "+$NIGHTLY" rustc --release --bin firmware \
    --target avr-none \
    -Z build-std=core \
    -Z build-std-features=compiler-builtins-mem \
    -- --emit=llvm-ir -o "$build_dir/target/firmware_avr.ll"

status "retarget avr-none -> i4004 (header rewrite + safety guards)"
python3 "$retarget" "$build_dir/target/firmware_avr.ll" "$build_dir/firmware_i4004.ll"

status "done: $build_dir/firmware_i4004.ll"
wc -l "$build_dir/firmware_i4004.ll"
