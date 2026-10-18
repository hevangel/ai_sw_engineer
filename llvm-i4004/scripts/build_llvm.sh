#!/usr/bin/env bash
# Build LLVM with the experimental I4004 MC target.
#
# Runs INSIDE the ai-sw-engineer container. The caller mounts:
#   -v B:\ai_sw_engineer_mc:/workspace     (repo bind mount, slow)
#   -v ai-sw-llvm-build:/build             (fast Docker named volume)
#
# Strategy (never edit the submodule in place):
#   1. rsync the pinned submodule (/workspace/external/llvm-project) into the
#      fast volume at /build/llvm-src  (first time only; refreshed on request).
#   2. apply the I4004 overlay (scripts/apply_overlay.py) onto /build/llvm-src.
#   3. cmake + ninja into /build/llvm-obj, install into /workspace/llvm-i4004/install.
#
# Only the MC tools are built: llvm-mc, llvm-tblgen, FileCheck, not, count,
# llvm-readobj, llvm-objdump (+ the lit helpers).
set -euo pipefail

WORKSPACE="${WORKSPACE:-/workspace}"
BACKEND="$WORKSPACE/llvm-i4004"
SUBMODULE="$WORKSPACE/external/llvm-project"
BUILD_ROOT="${BUILD_ROOT:-/build}"
LLVM_SRC="$BUILD_ROOT/llvm-src"
LLVM_OBJ="$BUILD_ROOT/llvm-obj"
INSTALL="$BACKEND/install"
JOBS="${JOBS:-16}"

echo "==> I4004 LLVM build"
echo "    submodule: $SUBMODULE"
echo "    src copy : $LLVM_SRC"
echo "    obj dir  : $LLVM_OBJ"
echo "    install  : $INSTALL"
echo "    jobs     : $JOBS"

if [[ ! -d $SUBMODULE/llvm ]]; then
  echo "ERROR: $SUBMODULE/llvm not found. Checkout the submodule on the host first." >&2
  exit 1
fi

# 1. Sync submodule into the fast volume (skip .git to save space/time).
mkdir -p "$BUILD_ROOT"
if [[ "${REFRESH_SRC:-0}" == "1" || ! -d $LLVM_SRC/llvm ]]; then
  echo "==> syncing LLVM source into volume (this is slow once; cached after)"
  rsync -a --delete --exclude='.git' "$SUBMODULE/" "$LLVM_SRC/"
else
  echo "==> reusing cached LLVM source at $LLVM_SRC"
fi

# 2. Apply the overlay (idempotent).
echo "==> applying I4004 overlay"
python3 "$BACKEND/scripts/apply_overlay.py" "$LLVM_SRC"

# 3. Configure + build.
if [[ ! -f $LLVM_OBJ/build.ninja ]]; then
  echo "==> configuring (cmake)"
  cmake -G Ninja -S "$LLVM_SRC/llvm" -B "$LLVM_OBJ" \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLVM_ENABLE_ASSERTIONS=ON \
    -DLLVM_TARGETS_TO_BUILD="" \
    -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD="I4004" \
    -DLLVM_ENABLE_PROJECTS="" \
    -DLLVM_ENABLE_RUNTIMES="" \
    -DLLVM_INCLUDE_TESTS=ON \
    -DLLVM_INCLUDE_EXAMPLES=OFF \
    -DLLVM_INCLUDE_BENCHMARKS=OFF \
    -DLLVM_CCACHE_BUILD=ON \
    -DCMAKE_C_COMPILER=clang \
    -DCMAKE_CXX_COMPILER=clang++ \
    -DLLVM_USE_LINKER=lld \
    -DCMAKE_INSTALL_PREFIX="$INSTALL"
fi

echo "==> building MC tools"
cmake --build "$LLVM_OBJ" -j "$JOBS" --target \
  llvm-mc llvm-tblgen FileCheck not count llvm-readobj llvm-objdump

echo "==> installing tools into $INSTALL/bin"
mkdir -p "$INSTALL/bin"
for t in llvm-mc llvm-tblgen FileCheck not count llvm-readobj llvm-objdump; do
  if [[ -f $LLVM_OBJ/bin/$t ]]; then
    cp -f "$LLVM_OBJ/bin/$t" "$INSTALL/bin/"
  fi
done

echo "==> done. llvm-mc at $INSTALL/bin/llvm-mc"
"$INSTALL/bin/llvm-mc" --version | head -n 4 || true
echo "==> registered targets:"
"$INSTALL/bin/llvm-mc" --version | grep -A40 "Registered Targets" | grep -i i4004 || \
  echo "    (i4004 not listed - check build)"
