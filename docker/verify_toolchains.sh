#!/usr/bin/env bash
# Check that every toolchain the full image promises is present and names its
# target. Run at image build time (the build fails on a missing toolchain) and
# by hand inside the container. Prints one line per toolchain.
set -euo pipefail
toolchains="${AI_SW_TOOLCHAINS:-/opt/ai-sw}"
nightly="${RUST_NIGHTLY:-nightly-2026-10-15}"
failures=0
ok() { printf 'ok       %-10s %s\n' "$1" "$2"; }
bad() { printf 'MISSING  %-10s %s\n' "$1" "$2"; failures=$((failures + 1)); }

for chip in i4004 i8008 i8086; do
    mc="$toolchains/llvm-$chip/install/bin/llvm-mc"
    if [[ -x $mc ]] && "$mc" --version | grep -qi "$chip"; then
        ok "$chip" "$("$mc" --version | grep -m1 -i 'llvm version' | xargs) (llc, llvm-mc, llvm-objdump)"
    else
        bad "$chip" "$mc with the $chip target"
    fi
done

rustc_i8086="${I8086_RUSTC:-/build/rust-obj/x86_64-unknown-linux-gnu/stage1/bin/rustc}"
if [[ -x $rustc_i8086 ]] && "$rustc_i8086" --print target-list | grep -qx i8086-unknown-none \
   && [[ -f $("$rustc_i8086" --print sysroot)/lib/rustlib/src/rust/library/Cargo.toml ]]; then
    ok rust-i8086 "$("$rustc_i8086" --version) (i8086-unknown-none, library sources for -Zbuild-std)"
else
    bad rust-i8086 "$rustc_i8086 with i8086-unknown-none and its library sources"
fi

if rustc +mos --version >/dev/null 2>&1; then
    ok mos6502 "$(rustc +mos --version) (llvm-mos, llvm-mos-sdk)"
else
    bad mos6502 "rustc +mos (base image)"
fi

if rustc "+$nightly" --print target-list 2>/dev/null | grep -qx m68k-unknown-none-elf \
   && command -v m68k-linux-gnu-ld >/dev/null; then
    ok m68k "$(rustc "+$nightly" --version) (m68k-unknown-none-elf, binutils-m68k)"
else
    bad m68k "rustc +$nightly m68k-unknown-none-elf and m68k-linux-gnu-ld"
fi

[[ $failures -eq 0 ]] || { echo "$failures toolchain(s) missing" >&2; exit 1; }
