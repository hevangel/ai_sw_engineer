#!/usr/bin/env bash
# Build the llvm-mos6502 qualification smoke image (issue hevangel/ai_engineer#58).
#
# Runs INSIDE the ai-sw-engineer container (has the pinned rust-mos `mos`
# toolchain and llvm-mos llvm-mc), from the repository root:
#
#   llvm-mos6502/scripts/build.sh
#
# Outputs (all under llvm-mos6502/build/, gitignored):
#   crt0.o                        — our bare-metal startup (rt/crt0.s)
#   target/.../release/smoke      — linked ELF (mos-unknown-none)
#   smoke.bin                     — flat 32 KiB ROM image ($8000..$FFFF)
#   symbols.txt                   — llvm-nm of the ELF (window/sentinel addrs)
# Every stage prints the artifact size and SHA-256.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"   # llvm-mos6502/
root="$(cd "$here/.." && pwd)"
build="$here/build"
mkdir -p "$build"

status() { printf '==> %s\n' "$*"; }

status "1/4 crt0 (llvm-mc, -triple mos)"
llvm-mc -arch=mos -filetype=obj "$here/rt/crt0.s" -o "$build/crt0.o"
# Pass the object under a content-hashed name: cargo does not track files
# given via -C link-arg, and a plain relink re-hardlinks the cached deps
# artifact without re-running ld.lld. A new name changes RUSTFLAGS, which
# invalidates cargo's fingerprint on every crt0 content change.
crt0_hashed="$build/crt0-$(sha256sum "$build/crt0.o" | cut -d' ' -f1).o"
cp "$build/crt0.o" "$crt0_hashed"

status "2/4 Rust smoke -> ELF (cargo +mos, mos-unknown-none, -Z build-std=core)"
cd "$here/rust/smoke"
export CARGO_TARGET_DIR="$build/target"
RUSTFLAGS="-C link-arg=-nostartfiles \
 -C link-arg=$crt0_hashed \
 -C link-arg=-L$here/spec \
 -C link-arg=-L/usr/local/mos-platform/common/lib" \
  cargo +mos build --release --target mos-unknown-none -Z build-std=core
elf="$build/target/mos-unknown-none/release/smoke"

status "3/4 flat ROM image (0x8000..0xFFFF)"
llvm-objcopy -O binary "$elf" "$build/smoke.bin"
sz=$(wc -c < "$build/smoke.bin" | tr -d ' ')
[[ $sz -eq 32768 ]] || { echo "ERROR: image is $sz bytes, expected 32768" >&2; exit 1; }

status "4/4 symbols + hashes"
llvm-nm "$elf" > "$build/symbols.txt"
sha256sum "$build/smoke.bin"
printf 'RESULTS  ' && grep -E "RESULTS" "$build/symbols.txt" | head -1
printf 'DONE     ' && grep -E "DONE" "$build/symbols.txt" | head -1
printf 'text size: ' && llvm-objdump -h "$elf" | awk '$2==".text" {print $3}'
