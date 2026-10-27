#!/usr/bin/env bash
# A Rust crate -> one i8086 object.
#
#   build_rust.sh <crate_dir> <bin_name> <out_dir> [extra cargo args...]
#   (env EXTRA_RUSTFLAGS: extra rustc flags; LLC_FLAGS: extra llc options;
#    RUST_NIGHTLY: the toolchain, default below)
#
# The pinned rustc has no i8086 target, so the build takes the msp430 route
# (rust/README.md): msp430 has the same data layout as spec/abi.md D1 (16-bit
# pointers, a 16-bit largest alignment, 32- and 64-bit integers aligned to 2),
# so every layout rustc computes is the one our backend uses.
#
# 1. rustc builds `core` and the crate for msp430-none-elf (-Zbuild-std=core,
#    opt-level=z, panic=abort, fat LTO) and emits one whole-program LLVM IR
#    module;
# 2. the module's triple and data layout are replaced with ours, and the IR is
#    checked for what the backend does not implement (atomics, vectors, other address spaces, msp430 intrinsics; the msp430_intrcc
#    calling convention of `extern "msp430-interrupt"` is an interrupt handler, D5);
# 3. llc -mtriple=i8086 makes <out_dir>/<bin_name>.o (one section per function)
#    and .s for reading.
set -euo pipefail
crate="$(cd "${1:?crate_dir}" && pwd)"
bin="${2:?bin_name}"
out="${3:?out_dir}"
shift 3
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
llc="$backend/install/bin/llc"
NIGHTLY="${RUST_NIGHTLY:-nightly-2026-10-15}"
DATA_LAYOUT="e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16" # abi.md D1
mkdir -p "$out"
out="$(cd "$out" && pwd)"

echo "==> rustc msp430-none-elf IR: $crate ($bin)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/i8086-rust-build/$(basename "$crate")}"
# A stub linker lets rustc finish codegen (where --emit=llvm-ir happens).
export RUSTFLAGS="-C linker=/bin/true -C embed-bitcode=yes ${EXTRA_RUSTFLAGS:-}"
( cd "$crate" && cargo "+$NIGHTLY" rustc --release --bin "$bin" --target msp430-none-elf \
    -Z build-std=core "$@" -- --emit=llvm-ir -o "$out/${bin}_msp430.ll" )

echo "==> retarget to i8086"
src="$out/${bin}_msp430.ll"
fail=0
check() { # <description> <extended regex>
  if grep -Eq "$2" "$src"; then echo "ERROR: the module uses $1 (not implemented, see spec/codegen.md)" >&2; fail=1; fi
}
check "atomic operations" '\b(atomicrmw|cmpxchg|fence)\b|load atomic|store atomic'
check "vector types" '<[0-9]+ x '
check "address spaces other than 0" 'addrspace\([1-9]'
check "msp430 intrinsics" 'llvm\.msp430'
[[ $fail == 0 ]] || exit 1
sed -e "s|^target datalayout = .*|target datalayout = \"$DATA_LAYOUT\"|" \
    -e 's|^target triple = .*|target triple = "i8086-unknown-none"|' \
    "$src" > "$out/$bin.ll"

echo "==> llc"
"$llc" -mtriple=i8086 -O2 -function-sections -data-sections ${LLC_FLAGS:-} -filetype=obj "$out/$bin.ll" -o "$out/$bin.o"
"$llc" -mtriple=i8086 -O2 -function-sections -data-sections ${LLC_FLAGS:-} "$out/$bin.ll" -o "$out/$bin.s"
echo "==> $out/$bin.o"
