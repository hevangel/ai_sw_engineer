#!/usr/bin/env bash
# rt/obj/compiler_builtins.o: Rust's compiler_builtins (IEEE software floating point,
# 128-bit integer arithmetic) compiled for i8086 by our llc.
#
#   build_compiler_builtins.sh          write the object
#   build_compiler_builtins.sh --check  fail if the committed object differs
#
# rustc builds compiler_builtins as part of -Z build-std for the msp430 target (the
# route of build_rust.sh); with --emit=llvm-ir it leaves the crate's IR behind. That IR
# is retargeted and compiled with one section per function, so the linker keeps only
# the routines a program calls (__addsf3, __muldf3, __multi3, __udivti3, ...). The
# routines are weak definitions: a strong one in rt/ (memcpy...) wins. Needs the
# pinned nightly with rust-src and network access for the first build.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
llc="$backend/install/bin/llc"
NIGHTLY="${RUST_NIGHTLY:-nightly-2026-10-15}"
DATA_LAYOUT="e-m:e-p:16:16-p1:32:16:16:16-i8:8-i16:16-i32:16-i64:16-f32:16-f64:16-n8:16-a:8-S16" # abi.md D1
check=0
[[ ${1:-} == --check ]] && check=1
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export CARGO_TARGET_DIR="$tmp/target"
export RUSTFLAGS="-C linker=/bin/true -C embed-bitcode=yes --emit=llvm-ir,link"
I8086_RUSTC="${I8086_RUSTC:-/build/rust-obj/x86_64-unknown-linux-gnu/stage1/bin/rustc}"
if [[ -x $I8086_RUSTC ]]; then
  # a rustc with the i8086 target (scripts/rust_build.sh): compiler_builtins is built for it
  # from source (-Z build-std), without the C routines of compiler-rt
  target=i8086-unknown-none
  ( cd "$backend/rt/cb" && RUSTC="$I8086_RUSTC" cargo "+$NIGHTLY" rustc --release --lib --target $target       -Z build-std=core -- -o "$tmp/cb" ) >&2
else
  target=msp430-none-elf
  ( cd "$backend/rt/cb" && cargo "+$NIGHTLY" rustc --release --lib --target $target       -Z build-std=core -- -o "$tmp/cb" ) >&2
fi
ll="$(ls "$tmp"/target/$target/release/build/compiler_builtins/*/out/compiler_builtins-*.ll)"
sed -e "s|^target datalayout = .*|target datalayout = \"$DATA_LAYOUT\"|"     -e 's|^target triple = .*|target triple = "i8086-unknown-none"|' "$ll" > "$tmp/cb0.ll"
# the half/fp128 routines are neither needed nor compilable here
python3 "$here/ir_drop_functions.py" "$tmp/cb0.ll" "$tmp/cb.ll"
"$llc" -mtriple=i8086 -O2 -function-sections -data-sections -filetype=obj "$tmp/cb.ll" -o "$tmp/compiler_builtins.o"
dst="$backend/rt/obj/compiler_builtins.o"
if [[ $check == 1 ]]; then
  cmp -s "$tmp/compiler_builtins.o" "$dst" || { echo "stale: rt/obj/compiler_builtins.o" >&2; exit 1; }
else
  cp "$tmp/compiler_builtins.o" "$dst"
  echo "wrote rt/obj/compiler_builtins.o ($(wc -c <"$dst") bytes)"
fi
