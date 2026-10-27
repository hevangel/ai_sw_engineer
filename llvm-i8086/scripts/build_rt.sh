#!/usr/bin/env bash
# Build the runtime library (rt/) into rt/obj/*.o, one object per source:
# the assembly with llvm-mc, the LLVM IR with llc. The objects are committed
# (they are small and the tests link them); run_tests.sh checks they are fresh.
#
#   build_rt.sh          write rt/obj/*.o
#   build_rt.sh --check  fail if a committed object differs from a fresh build
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$BACKEND_DIR/install/bin"
RT="$BACKEND_DIR/rt"
CHECK=0
[[ ${1:-} == --check ]] && CHECK=1
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$RT/obj"

fail=0
build() { # <name> <command producing $TMP/<name>.o>
  local name=$1
  if [[ $CHECK == 1 ]]; then
    cmp -s "$TMP/$name.o" "$RT/obj/$name.o" || { echo "stale: rt/obj/$name.o" >&2; fail=1; }
  else
    cp "$TMP/$name.o" "$RT/obj/$name.o"
    echo "wrote rt/obj/$name.o"
  fi
}

for s in "$RT"/asm/*.s; do
  name="$(basename "$s" .s)"
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$s" -o "$TMP/$name.o"
  build "$name"
done
for ll in "$RT"/ir/*.ll; do
  name="$(basename "$ll" .ll)"
  "$BIN/llc" -mtriple=i8086 -O2 -filetype=obj "$ll" -o "$TMP/$name.o"
  build "$name"
done

# The same objects as one archive, for `i8086-ld ... rt/librt.a` (deterministic: no dates or owners).
if command -v llvm-ar >/dev/null; then
  # compiler_builtins.o goes last: its weak memcpy and friends must not be taken
  # before this library's own (members are tried in archive order).
  (cd "$RT/obj" && llvm-ar rcsD "$TMP/librt.a" $(ls *.o | grep -v '^compiler_builtins\.o$' | LC_ALL=C sort) compiler_builtins.o)
  if [[ $CHECK == 1 ]]; then
    cmp -s "$TMP/librt.a" "$RT/librt.a" || { echo "stale: rt/librt.a" >&2; fail=1; }
  else
    cp "$TMP/librt.a" "$RT/librt.a"
    echo "wrote rt/librt.a"
  fi
fi
exit $fail
