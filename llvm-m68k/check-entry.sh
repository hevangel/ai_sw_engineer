#!/usr/bin/env bash
# Entry-point and section check for a built m68k ELF.
# Usage (host): docker run --rm -v <repo>:/workspace -w /workspace \
#   ai-hw-engineer:latest bash /workspace/llvm-m68k/check-entry.sh [elf]
set -euo pipefail
apt-get update -qq >/dev/null 2>&1 || true
apt-get install -y -qq binutils-m68k-linux-gnu >/dev/null 2>&1
ELF="${1:-/workspace/llvm-m68k/build/target/m68k-unknown-none-elf/release/mini}"
m68k-linux-gnu-objdump -h "$ELF" | sed -n '/Idx/,$p'
echo "--- _start and friends:"
m68k-linux-gnu-nm "$ELF" | grep -E " (_start|smokem68k|main)" || true
