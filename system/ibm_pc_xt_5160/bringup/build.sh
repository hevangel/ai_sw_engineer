#!/usr/bin/env bash
# Build the hand-written bring-up baseline boot sector (NOT compiler output).
# Usage: bringup/build.sh   -> bringup/boot_hello.bin (512 bytes, AA55-signed)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if ! command -v nasm >/dev/null 2>&1; then
    echo "ERROR: nasm is required (see spec/toolchain_audit.md). Version pin: at implementation." >&2
    exit 1
fi
echo "==> nasm: $(nasm -v)"

nasm -f bin "$HERE/boot_hello.asm" -o "$HERE/boot_hello.bin"

size=$(wc -c < "$HERE/boot_hello.bin")
[[ $size -eq 512 ]] || { echo "ERROR: expected 512 bytes, got $size" >&2; exit 1; }
sig=$(xxd -s 510 -l 2 -p "$HERE/boot_hello.bin")
[[ $sig == "55aa" ]] || { echo "ERROR: bad boot signature: $sig" >&2; exit 1; }

echo "==> ok: boot_hello.bin (512 bytes, signature 55 AA)"
sha256sum "$HERE/boot_hello.bin"
