#!/usr/bin/env bash
# Disassemble a range of the mini-O0 ELF with the m68k GNU objdump.
# Usage (host): docker run --rm -v <repo>:/workspace -w /workspace \
#   ai-hw-engineer:latest bash /workspace/llvm-m68k/disasm-range.sh [start] [stop]
set -euo pipefail
apt-get update -qq >/dev/null 2>&1 || true
apt-get install -y -qq binutils-m68k-linux-gnu >/dev/null 2>&1
START="${1:-0xfa0}"
STOP="${2:-0xfe8}"
m68k-linux-gnu-objdump -d \
  --start-address="$START" --stop-address="$STOP" \
  /workspace/llvm-m68k/build/target/m68k-unknown-none-elf/release/mini
