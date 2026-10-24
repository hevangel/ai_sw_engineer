#!/usr/bin/env bash
# Rust crate -> one i8008 object (spec/codegen.md sec.1).
#
#   build_rust.sh <crate_dir> <bin_name> <out_dir> [extra cargo args...]
#   (env LLC_FLAGS: extra llc options; EXTRA_RUSTFLAGS: extra rustc flags)
#
# 1. rustc (pinned nightly) for avr-none with build-std=core, opt-level=z,
#    panic=abort and fat LTO emits one whole-program LLVM IR module;
# 2. rust/retarget_ir.py rewrites it for i8008 (and refuses FP, i128, AVR
#    intrinsics, non-default data address spaces);
# 3. llc -mtriple=i8008 -function-sections makes <out_dir>/<bin_name>.o.
# Also writes <bin_name>.s for reading. Runs in the ai-sw-engineer image.
set -euo pipefail
crate="$(cd "${1:?crate_dir}" && pwd)"
bin="${2:?bin_name}"
out="${3:?out_dir}"
shift 3
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
llc="$backend/install/bin/llc"
NIGHTLY="${RUST_NIGHTLY:-nightly-2026-10-15}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

echo "==> rustc avr-none IR: $crate ($bin)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/i8008-rust-build/$(basename "$crate")}"
# A stub linker lets rustc finish codegen (where --emit=llvm-ir happens).
# Keep loop counters at the width the source gives them (u8 stays u8).
export RUSTFLAGS="-C target-cpu=atmega328p -C opt-level=z -C panic=abort -C lto=fat -C embed-bitcode=yes -C linker=/bin/true -C llvm-args=-indvars-widen-indvars=false ${EXTRA_RUSTFLAGS:-}"
( cd "$crate" && cargo "+$NIGHTLY" rustc --release --bin "$bin" --target avr-none \
    -Z build-std=core -Z build-std-features=compiler-builtins-mem "$@" \
    -- --emit=llvm-ir -o "$out/${bin}_avr.ll" )

echo "==> retarget to i8008"
python3 "$backend/rust/retarget_ir.py" "$out/${bin}_avr.ll" "$out/$bin.ll"

echo "==> llc"
# LLC_FLAGS: per-image codegen options, e.g. -i8008-data-page.
"$llc" -mtriple=i8008 -O2 -function-sections ${LLC_FLAGS:-} -filetype=obj "$out/$bin.ll" -o "$out/$bin.o"
"$llc" -mtriple=i8008 -O2 -function-sections ${LLC_FLAGS:-} "$out/$bin.ll" -o "$out/$bin.s"
echo "==> $out/$bin.o ($(grep -c '^[A-Za-z_].*:' "$out/$bin.s") labels)"
