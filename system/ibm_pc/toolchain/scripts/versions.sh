#!/usr/bin/env bash
# Print the versions of everything the profile depends on (run in the ai-sw-engineer image,
# from the repository root, after llvm-i8086/scripts/rust_build.sh).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
RUSTC="${I8086_RUSTC:-/build/rust-obj/x86_64-unknown-linux-gnu/stage1/bin/rustc}"
echo "image:        $(cat /home/mos/TOOLCHAIN_VERSIONS.txt 2>/dev/null | head -1)"
echo "host rustc:   $(rustc +1.99.1 --version)"
echo "host cargo:   $(cargo +1.99.1 --version)"
echo "i8086 rustc:  $("$RUSTC" --version -v 2>/dev/null | sed -n '1p;3p;6p' | tr '\n' ' ')"
echo "i8086 target: $("$RUSTC" --print target-list 2>/dev/null | grep -c '^i8086-unknown-none$') (i8086-unknown-none in the target list)"
echo "rust patch:   sha256 $(sha256sum "$ROOT/llvm-i8086/rust/patches/apply_i8086_target.py" | cut -c1-16)"
echo "llvm commit:  $(git -c safe.directory='*' -C "$ROOT" submodule status external/llvm-project 2>/dev/null | awk '{print $1, $3}')"
echo "llc:          $("$ROOT/llvm-i8086/install/bin/llc" --version | sed -n '2p;3p' | tr '\n' ' ')"
echo "llvm-mc:      $("$ROOT/llvm-i8086/install/bin/llvm-mc" --version | grep -i 'i8086' | tr -s ' ')"
echo "i8086-ld:     $(grep -m1 '^version' "$ROOT/llvm-i8086/asm/Cargo.toml") (built from the working tree)"
echo "binutils:     $(objdump --version | head -1)"
echo "capstone:     $(python3 -c 'import capstone; print(capstone.__version__)')"
echo "gcc/cmake:    $(gcc --version | head -1) / $(cmake --version | head -1) / ninja $(ninja --version)"
echo "martypc:      $(git -c safe.directory='*' -C "$ROOT" submodule status external/martypc 2>/dev/null | awk '{print $1, $3}')"
