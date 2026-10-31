#!/usr/bin/env bash
# Build LLVM with the mos6502 target into llvm-mos6502/install/.
# Applies target/ onto a worktree of external/llvm-project (the submodule itself is never edited).
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$BACKEND_DIR/.." && pwd)"
SRC="$BACKEND_DIR/build/llvm-project"
INSTALL="$BACKEND_DIR/install"

echo "==> llvm-mos6502: build_llvm.sh not implemented yet" >&2
echo "    plan: worktree $ROOT/external/llvm-project -> $SRC, copy target/ into llvm/lib/Target/," >&2
echo "    cmake -G Ninja -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=<Chip> -DLLVM_TARGETS_TO_BUILD= -DCMAKE_INSTALL_PREFIX=$INSTALL" >&2
exit 2
