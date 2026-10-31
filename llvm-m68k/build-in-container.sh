#!/usr/bin/env bash
# Rebuild the per-opt mini ELFs/images, then disassemble the O0 ELF at
# the range the executor trace pointed at. Runs in the container:
#   docker run --rm -v <repo>:/workspace -w /workspace \
#     ai-hw-engineer:latest bash /workspace/llvm-m68k/build-in-container.sh mini
set -euo pipefail

apt-get update -qq >/dev/null 2>&1 || true
apt-get install -y -qq binutils-m68k-linux-gnu >/dev/null 2>&1

rustup toolchain install nightly-2026-10-15 --component rust-src --component llvm-tools --profile minimal 2>&1 | tail -1
SYSROOT="$(rustc +nightly-2026-10-15 --print sysroot)"
export PATH="$SYSROOT/lib/rustlib/x86_64-unknown-linux-gnu/bin:$PATH"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"   # llvm-m68k/
build="$here/build"
mkdir -p "$build"

for opt in 0 1 2 s; do
    echo "==> mini -O$opt"
    cd "$here/rust/smoke"
    export CARGO_TARGET_DIR="$build/target"
    CARGO_TARGET_DIR="$build/target" \
    CARGO_PROFILE_RELEASE_OPT_LEVEL="$opt" \
    RUSTFLAGS="-C target-cpu=M68000 \
     -C linker=m68k-linux-gnu-ld -C linker-flavor=ld \
     -C link-arg=-e -C link-arg=_start -C link-arg=-Ttext=0x400" \
      cargo "+nightly-2026-10-15" build --release --target m68k-unknown-none-elf \
        -Z build-std=core --bin mini
    elf="$build/target/m68k-unknown-none-elf/release/mini"
    cp "$elf" "$build/mini-O$opt.elf"
    llvm-objcopy -O binary "$elf" "$build/mini-O$opt.bin"
    # Record the real entry: the linker layout does not guarantee
    # _start lands at 0x400 for every opt level.
    llvm-nm "$elf" | awk '$3=="_start" {print "start=0x"$1}' | tee "$build/mini-O$opt.start"
    sha256sum "$build/mini-O$opt.bin"
done

echo "==> disasm mini-O0 0xfa0..0xfe0"
m68k-linux-gnu-objdump -d --start-address=0xfa0 --stop-address=0xfe0 "$build/mini-O0.elf" | tail -n +8
echo "==> mini sweep build OK"
