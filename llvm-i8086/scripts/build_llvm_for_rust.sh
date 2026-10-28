#!/usr/bin/env bash
# Build and install the LLVM libraries (X86 for the host, I8086 for the target) and
# llvm-config that a rustc with an i8086 target links against (rust/README.md).
#
# Runs in the ai-sw-engineer container, after build_llvm.sh has put the sources with
# the I8086 overlay in the volume:
#   docker run --rm -v <repo>:/workspace -v ai-sw-llvm-build:/build -w /workspace \
#     ai-sw-engineer:latest llvm-i8086/scripts/build_llvm_for_rust.sh
set -euo pipefail

WORKSPACE="${WORKSPACE:-/workspace}"
BACKEND="$WORKSPACE/llvm-i8086"
BUILD_ROOT="${BUILD_ROOT:-/build}"
LLVM_SRC="$BUILD_ROOT/llvm-src"
LLVM_OBJ="$BUILD_ROOT/llvm-obj-rust"
INSTALL="$BUILD_ROOT/llvm-rust-install"
JOBS="${JOBS:-24}"

[[ -d $LLVM_SRC/llvm ]] || { echo "ERROR: run build_llvm.sh first (it fills $LLVM_SRC)" >&2; exit 1; }
python3 "$BACKEND/scripts/apply_overlay.py" "$LLVM_SRC"
export CC="$(command -v gcc)" CXX="$(command -v g++)"

if [[ ! -f $LLVM_OBJ/build.ninja ]]; then
  cmake -G Ninja -S "$LLVM_SRC/llvm" -B "$LLVM_OBJ" \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLVM_ENABLE_ASSERTIONS=OFF \
    -DLLVM_TARGETS_TO_BUILD="X86" \
    -DLLVM_EXPERIMENTAL_TARGETS_TO_BUILD="I8086" \
    -DLLVM_ENABLE_PROJECTS="" -DLLVM_ENABLE_RUNTIMES="" \
    -DLLVM_INCLUDE_TESTS=OFF -DLLVM_INCLUDE_EXAMPLES=OFF -DLLVM_INCLUDE_BENCHMARKS=OFF \
    -DLLVM_INCLUDE_DOCS=OFF -DLLVM_BUILD_TOOLS=OFF \
    -DLLVM_ENABLE_ZLIB=OFF -DLLVM_ENABLE_ZSTD=OFF -DLLVM_ENABLE_TERMINFO=OFF \
    -DLLVM_ENABLE_LIBXML2=OFF -DLLVM_ENABLE_LIBEDIT=OFF \
    -DLLVM_CCACHE_BUILD=ON \
    -DCMAKE_C_COMPILER="$CC" -DCMAKE_CXX_COMPILER="$CXX" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL"
fi
cmake --build "$LLVM_OBJ" -j "$JOBS" --target llvm-config
cmake --build "$LLVM_OBJ" -j "$JOBS" --target install-llvm-headers install-llvm-libraries install-cmake-exports
mkdir -p "$INSTALL/bin" && cp -f "$LLVM_OBJ/bin/llvm-config" "$INSTALL/bin/"
"$INSTALL/bin/llvm-config" --version --targets-built
