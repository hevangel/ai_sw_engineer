#!/usr/bin/env bash
# Build apple_basic: host check, host LLVM IR, then the target image through llvm-mos6502.
# Usage: scripts/build.sh [host|ir|target|all]   (default: all)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

stage="${1:-all}"
mkdir -p "$BUILD_DIR"
cd "$TITLE_DIR"

if [[ $stage == host || $stage == all ]]; then
    status "host build"
    cargo build --release
fi

if [[ $stage == ir || $stage == all ]]; then
    status "host LLVM IR -> build/host/"
    mkdir -p "$BUILD_DIR/host"
    cargo rustc --release --lib -- --emit=llvm-ir
    target_dir=$(cargo metadata --format-version 1 --no-deps |
        python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
    ll=$(ls -t "$target_dir"/release/deps/software-*.ll 2> /dev/null | head -n1)
    [[ -n $ll ]] || fail "rustc produced no .ll under $target_dir/release/deps"
    cp "$ll" "$BUILD_DIR/host/$SOFTWARE.ll"
fi

if [[ $stage == target || $stage == all ]]; then
    hook="$BACKEND_DIR/scripts/build_image.sh"
    if [[ ! -x $hook ]]; then
        status "SKIP target image: $hook not available yet (backend llvm-$CHIP not ready)"
        exit 0
    fi
    status "target image via llvm-$CHIP"
    # Contract: build_image.sh <title_dir> <out_dir> <linker_script>
    # Produces <out_dir>/$SOFTWARE.{ll,s,elf,bin,map}. Exit 2 = backend not implemented yet.
    rc=0
    "$hook" "$TITLE_DIR" "$BUILD_DIR/target" "$PLATFORM_DIR/link.ld" || rc=$?
    if [[ $rc -eq 2 ]]; then
        status "SKIP target image: llvm-$CHIP build_image.sh not implemented yet"
        exit 0
    fi
    [[ $rc -eq 0 ]] || fail "backend build failed (exit $rc)"
    image="$BUILD_DIR/target/$SOFTWARE.bin"
    [[ -f $image ]] || fail "backend did not produce $image"
    size=$(wc -c < "$image" | tr -d ' ')
    if [[ $ROM_BUDGET -gt 0 ]]; then
        status "image size: $size / $ROM_BUDGET bytes"
        [[ $size -le $ROM_BUDGET ]] || fail "image exceeds ROM budget by $((size - ROM_BUDGET)) bytes"
    else
        status "image size: $size bytes (ROM_BUDGET unset)"
    fi
fi
