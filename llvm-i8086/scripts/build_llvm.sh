#!/usr/bin/env bash
# Build LLVM with the i8086 target into llvm-i8086/install/.
# Applies target/ onto a worktree of external/llvm-project (the submodule itself is never edited).
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$BACKEND_DIR/.." && pwd)"
SRC="$BACKEND_DIR/build/llvm-project"
INSTALL="$BACKEND_DIR/install"

echo "==> llvm-i8086: build_llvm.sh not implemented yet" >&2
echo "    plan: worktree $ROOT/external/llvm-project -> $SRC, copy target/ into llvm/lib/Target/," >&2
echo "    cmake -G Ninja -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD=<Chip> -DLLVM_TARGETS_TO_BUILD= -DCMAKE_INSTALL_PREFIX=$INSTALL" >&2
exit 2
