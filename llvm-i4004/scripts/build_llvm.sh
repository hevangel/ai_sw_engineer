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
  rm -rf "$LLVM_SRC"
  mkdir -p "$LLVM_SRC"
  # Copy the tree except .git metadata (rsync is not in the image; tar is).
  ( cd "$SUBMODULE" && tar --exclude='./.git' -cf - . ) | ( cd "$LLVM_SRC" && tar -xf - )
  # The host (Windows git) may have checked the submodule out with CRLF line
  # endings. cmake executes some of these as /bin/sh scripts (config.guess,
  # config.sub) and a trailing CR breaks them. Strip CR from the scripts cmake
  # runs. (Targeted: shell + autoconf helper scripts only, never binaries.)
  echo "==> stripping CR from LLVM shell/helper scripts (Windows checkout fix)"
  find "$LLVM_SRC" \( -name '*.sh' -o -name 'config.guess' -o -name 'config.sub' \
       -o -name 'config.status' -o -name '*.guess' -o -name '*.sub' \) -type f \
       -exec sed -i 's/\r$//' {} + 2>/dev/null || true
else
  echo "==> reusing cached LLVM source at $LLVM_SRC"
fi

# 2. Apply the overlay (idempotent).
echo "==> applying I4004 overlay"
python3 "$BACKEND/scripts/apply_overlay.py" "$LLVM_SRC"

# 2b. Force the I4004 target objects to recompile.
# apply_overlay.py rewrites the target sources via shutil.rmtree + copytree,
# which does NOT reliably advance file mtimes in a way ninja trusts, so an
# incremental build SILENTLY SKIPS recompiling changed backend files (e.g.
# I4004ExpandPseudos.cpp) -- you get an llc that relinks but still runs the OLD
# pass. Delete the target's objects so ninja must rebuild them from the fresh
# overlay sources. Harmless on a clean/first build (nothing to delete).
if [[ -d "$LLVM_OBJ" ]]; then
  find "$LLVM_OBJ" -path '*Target/I4004*' -name '*.o' -delete 2>/dev/null || true
fi

# 3. Configure + build.
#
# The toolchain image's bare `clang`/`clang++` (/usr/local/bin, ahead of
# /usr/bin on PATH) is the llvm-mos cross-compiler: it defaults to
# `-target mos-unknown-unknown` and cannot link a host x86-64 binary (it pulls
# the 6502 crt0). So the HOST LLVM build must use the Debian gcc/g++, never the
# default clang. We both pass -DCMAKE_*_COMPILER and export CC/CXX so cmake's
# own internal compiler probes (and any sub-build it spawns) also pick gcc, not
# a re-detected clang. This is scoped to this script's process only: the global
# env is untouched, so `rustc +mos` / llvm-mos-sdk builds that legitimately want
# the mos clang are unaffected.
HOST_CC="$(command -v gcc || true)"
HOST_CXX="$(command -v g++ || true)"
if [[ -z $HOST_CC || -z $HOST_CXX ]]; then
  echo "ERROR: host gcc/g++ not found; the image should provide build-essential." >&2
  exit 1
fi
# Guard: refuse to proceed if these somehow resolve to a non-host (mos) target.
if "$HOST_CC" -dumpmachine 2>/dev/null | grep -qi '^mos'; then
  echo "ERROR: $HOST_CC targets mos, not the host; refusing to build LLVM with it." >&2
  exit 1
fi
export CC="$HOST_CC" CXX="$HOST_CXX"
echo "==> host compilers: CC=$CC ($("$HOST_CC" -dumpmachine)) CXX=$CXX"

if [[ "${CLEAN_OBJ:-0}" == "1" || ! -f $LLVM_OBJ/build.ninja ]]; then
  echo "==> configuring (cmake)"
  if [[ "${CLEAN_OBJ:-0}" == "1" ]]; then
    # Drop the stale cache so cmake reconfigures from scratch (targeted file
    # deletes only; no recursive remove).
    rm -f "$LLVM_OBJ/CMakeCache.txt" || true
    find "$LLVM_OBJ" -name CMakeCache.txt -delete 2>/dev/null || true
  fi
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
    -DCMAKE_C_COMPILER="$HOST_CC" \
    -DCMAKE_CXX_COMPILER="$HOST_CXX" \
    -DCMAKE_INSTALL_PREFIX="$INSTALL"
fi

echo "==> building MC + CodeGen tools"
cmake --build "$LLVM_OBJ" -j "$JOBS" --target \
  llvm-mc llc llvm-tblgen FileCheck not count llvm-readobj llvm-objdump

echo "==> installing tools into $INSTALL/bin"
mkdir -p "$INSTALL/bin"
for t in llvm-mc llc llvm-tblgen FileCheck not count llvm-readobj llvm-objdump; do
  if [[ -f $LLVM_OBJ/bin/$t ]]; then
    cp -f "$LLVM_OBJ/bin/$t" "$INSTALL/bin/"
  fi
done

echo "==> done. llvm-mc at $INSTALL/bin/llvm-mc"
"$INSTALL/bin/llvm-mc" --version | head -n 4 || true
echo "==> registered targets:"
"$INSTALL/bin/llvm-mc" --version | grep -A40 "Registered Targets" | grep -i i4004 || \
  echo "    (i4004 not listed - check build)"
