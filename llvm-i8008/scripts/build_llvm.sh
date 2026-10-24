#!/usr/bin/env bash
# Build llc + llvm-mc (+ lit helpers) with the experimental I8008 target.
#
# Runs inside the ai-sw-engineer container:
#   docker run --rm -v <repo>:/workspace -v ai-sw-llvm-build:/build \
#     -w /workspace ai-sw-engineer:latest llvm-i8008/scripts/build_llvm.sh
#
# The pinned submodule (external/llvm-project) is never edited. Its copy in
# the build volume (/build/llvm-src, shared with llvm-i4004) gets the I8008
# overlay; the object tree is separate (/build/llvm-obj-i8008).
set -euo pipefail

WORKSPACE="${WORKSPACE:-/workspace}"
BACKEND="$WORKSPACE/llvm-i8008"
SUBMODULE="$WORKSPACE/external/llvm-project"
BUILD_ROOT="${BUILD_ROOT:-/build}"
LLVM_SRC="$BUILD_ROOT/llvm-src"
LLVM_OBJ="$BUILD_ROOT/llvm-obj-i8008"
INSTALL="$BACKEND/install"
JOBS="${JOBS:-16}"
TOOLS=(llvm-mc llc llvm-tblgen FileCheck not count llvm-readobj llvm-objdump)

echo "==> I8008 LLVM build (src $LLVM_SRC, obj $LLVM_OBJ, jobs $JOBS)"

if [[ ! -d $LLVM_SRC/llvm ]]; then
  [[ -d $SUBMODULE/llvm ]] || { echo "ERROR: $SUBMODULE/llvm missing; check out the submodule" >&2; exit 1; }
  echo "==> copying the LLVM source into the volume (once)"
  mkdir -p "$LLVM_SRC"
  ( cd "$SUBMODULE" && tar --exclude='./.git' -cf - . ) | ( cd "$LLVM_SRC" && tar -xf - )
  # A Windows checkout may have CRLF shell helpers that cmake runs.
  find "$LLVM_SRC" \( -name '*.sh' -o -name 'config.guess' -o -name 'config.sub' \) \
    -type f -exec sed -i 's/\r$//' {} + 2>/dev/null || true
fi

echo "==> applying the I8008 overlay"
python3 "$BACKEND/scripts/apply_overlay.py" "$LLVM_SRC"
# copytree does not advance mtimes in a way ninja trusts: force a rebuild of
# the target's objects so a changed backend source is never silently skipped.
if [[ -d $LLVM_OBJ ]]; then
  find "$LLVM_OBJ" -path '*Target/I8008*' -name '*.o' -delete 2>/dev/null || true
fi

# The image's default clang is the llvm-mos cross compiler; build with gcc.
export CC="$(command -v gcc)" CXX="$(command -v g++)"

if [[ ! -f $LLVM_OBJ/build.ninja ]]; then
  echo "==> configuring"
  cmake -G Ninja -S "$LLVM_SRC/llvm" -B "$LLVM_OBJ" \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLVM_ENABLE_ASSERTIONS=ON \
    -DLLVM_TARGETS_TO_BUILD="" \
    -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD="I8008" \
    -DLLVM_ENABLE_PROJECTS="" \
    -DLLVM_ENABLE_RUNTIMES="" \
    -DLLVM_INCLUDE_TESTS=ON \
    -DLLVM_INCLUDE_EXAMPLES=OFF \
    -DLLVM_INCLUDE_BENCHMARKS=OFF \
    -DLLVM_CCACHE_BUILD=ON \
    -DCMAKE_C_COMPILER="$CC" \
    -DCMAKE_CXX_COMPILER="$CXX" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL"
fi

echo "==> building ${TOOLS[*]}"
cmake --build "$LLVM_OBJ" -j "$JOBS" --target "${TOOLS[@]}"

mkdir -p "$INSTALL/bin"
for t in "${TOOLS[@]}"; do cp -f "$LLVM_OBJ/bin/$t" "$INSTALL/bin/"; done
"$INSTALL/bin/llc" --version | grep -i i8008 || { echo "ERROR: i8008 not registered" >&2; exit 1; }
echo "==> done: $INSTALL/bin"
