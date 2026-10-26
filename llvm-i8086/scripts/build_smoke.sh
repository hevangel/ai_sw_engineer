#!/usr/bin/env bash
# Build the Rust smoke program objects the emulator tests link:
# tests/smoke/<crate>/<crate>.o (rustc as msp430, retargeted, our llc), and the
# BIOS-free test stub tests/smoke/stub_hal.o.
#
#   build_smoke.sh          write the objects
#   build_smoke.sh --check  fail if a committed object differs from a fresh build
#
# Needs the pinned nightly with rust-src and network access for the first
# build (cargo fetches core's dependencies). Run in the ai-sw-engineer image.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
check=0
[[ ${1:-} == --check ]] && check=1
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fail=0

emit() { # <fresh object> <committed object>
  if [[ $check == 1 ]]; then
    cmp -s "$1" "$2" || { echo "stale: ${2#"$backend"/}" >&2; fail=1; }
  else
    cp "$1" "$2"; echo "wrote ${2#"$backend"/}"
  fi
}

for crate in "$backend"/tests/smoke/*/; do
  [[ -f $crate/Cargo.toml ]] || continue
  name="$(basename "$crate")"
  "$here/build_rust.sh" "$crate" "$name" "$tmp/$name" >&2
  emit "$tmp/$name/$name.o" "$crate/$name.o"
done
"$backend/install/bin/llvm-mc" -triple=i8086 -filetype=obj "$backend/tests/smoke/stub_hal.s" -o "$tmp/stub_hal.o"
emit "$tmp/stub_hal.o" "$backend/tests/smoke/stub_hal.o"
exit $fail
