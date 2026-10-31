#!/usr/bin/env bash
# Disassemble a range of a built m68k ELF.
# Usage (host): docker run --rm -v <repo>:/workspace -w /workspace \
#   ai-hw-engineer:latest bash /workspace/llvm-m68k/disasm2.sh <elf> <start> <stop>
set -euo pipefail
apt-get update -qq >/dev/null 2>&1 || true
apt-get install -y -qq binutils-m68k-linux-gnu >/dev/null 2>&1
m68k-linux-gnu-objdump -d --start-address="$2" --stop-address="$3" "$1"
