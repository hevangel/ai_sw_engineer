#!/usr/bin/env bash
# Build (or rebuild) the rustc with the i8086 target in the build volume; see rust/README.md.
#   docker run --rm -v <repo>:/workspace -v ai-sw-llvm-build:/build ai-sw-engineer:latest \
#       llvm-i8086/scripts/rust_build.sh [x.py arguments]
set -euo pipefail
WORKSPACE="${WORKSPACE:-/workspace}"
SRC="${RUST_SRC:-/build/rust-src}"
PIN=7df81381d027710708209f36b98d11b1c88f8d2e   # rust-lang/rust of nightly-2026-10-15
if [[ ! -d $SRC/compiler ]]; then
  mkdir -p "$SRC" && cd "$SRC"
  git init -q . && git remote add origin https://github.com/rust-lang/rust.git
  git fetch --depth 1 origin "$PIN" && git checkout -q FETCH_HEAD
fi
python3 "$WORKSPACE/llvm-i8086/rust/patches/apply_i8086_target.py" "$SRC"
cp "$WORKSPACE/llvm-i8086/rust/patches/bootstrap.toml" "$SRC/bootstrap.toml"
cd "$SRC"
# the stage0 compiler downloaded by bootstrap does not know the new target
export BOOTSTRAP_SKIP_TARGET_SANITY=1
if [[ $# -eq 0 ]]; then
  set -- build --stage 1 library --target x86_64-unknown-linux-gnu,i8086-unknown-none
fi
python3 x.py "$@" -j "${JOBS:-28}"
