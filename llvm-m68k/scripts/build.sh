#!/usr/bin/env bash
# Build the llvm-m68k qualification smoke image (issue hevangel/ai_engineer#64).
#
# Runs INSIDE the ai-sw-engineer container, from the repository root.
# Requires binutils-m68k-linux-gnu (GNU m68k ld + objdump): pinned in the
# Dockerfile; on the current prebuilt image install it ad hoc:
#   apt-get update -qq && apt-get install -y -qq binutils-m68k-linux-gnu
#
#   llvm-m68k/scripts/build.sh
#
# Outputs (under llvm-m68k/build/, gitignored):
#   target/.../release/smoke   — linked ELF (m68k-unknown-none-elf, .text at 0x400)
#   smoke.bin                  — flat image ($00000400.., for the executor)
#   symbols.txt, smoke.elf kept for the checks
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # llvm-m68k/
build="$here/build"
mkdir -p "$build"

status() { printf '==> %s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

command -v m68k-linux-gnu-ld >/dev/null || fail "m68k-linux-gnu-ld not found (apt-get install binutils-m68k-linux-gnu)"
command -v m68k-linux-gnu-objdump >/dev/null || fail "m68k-linux-gnu-objdump not found"

TOOLCHAIN="${RUST_NIGHTLY:-nightly-2026-10-15}"   # the repo's pinned nightly (rust-src for -Z build-std)

status "1/3 Rust smoke -> ELF (cargo +$TOOLCHAIN, m68k-unknown-none-elf, -Z build-std=core, target-cpu=M68000)"
cd "$here/rust/smoke"
export CARGO_TARGET_DIR="$build/target"
RUSTFLAGS="-C target-cpu=M68000 \
 -C linker=m68k-linux-gnu-ld -C linker-flavor=ld \
 -C link-arg=-e -C link-arg=_start -C link-arg=-Ttext=0x400" \
  cargo "+$TOOLCHAIN" build --release --target m68k-unknown-none-elf -Z build-std=core
elf="$build/target/m68k-unknown-none-elf/release/smoke"
cp "$elf" "$build/smoke.elf"

status "2/3 flat image (from 0x00000400)"
llvm-objcopy -O binary "$elf" "$build/smoke.bin"

status "3/3 symbols + hashes"
llvm-nm "$elf" > "$build/symbols.txt"
grep -E " (_start|smokem68k)" "$build/symbols.txt" | head -3 || true
sha256sum "$build/smoke.bin"
start=$(llvm-nm "$elf" | awk '$3=="_start" {print $1}')
[[ -n $start ]] || fail "_start not in symbols"
printf 'entry _start at: 0x%s (must be 0x00000400)\n' "$start"
[[ "0x$start" == "0x00000400" ]] || fail "entry is not at 0x400"
status "build OK"
