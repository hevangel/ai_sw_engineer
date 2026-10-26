#!/usr/bin/env bash
# Build the object files that system/ibm_pc/emulator/tests/i8086_codegen.rs and i8086_boot.rs
# runs: one ELF relocatable per tests/run/*.ll, by `llc -filetype=obj`.
#
#   gen_run_objects.sh          write tests/run/*.o
#   gen_run_objects.sh --check  fail if a committed object differs from a
#                               fresh build (used by run_tests.sh)
#
# Each object is also assembled the other way round (llc -> text -> llvm-mc)
# and the two must agree byte for byte in every section, which ties the
# instruction printer, the parser and the encoder together on real compiler
# output.
set -euo pipefail
BACKEND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$BACKEND_DIR/install/bin"
RUN="$BACKEND_DIR/tests/run"
CHECK=0
[[ ${1:-} == --check ]] && CHECK=1
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail=0
for ll in "$RUN"/*.ll; do
  name="$(basename "$ll" .ll)"
  "$BIN/llc" -mtriple=i8086 -O2 -filetype=obj "$ll" -o "$TMP/$name.o"
  "$BIN/llc" -mtriple=i8086 -O2 "$ll" -o "$TMP/$name.s"
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$TMP/$name.s" -o "$TMP/$name.mc.o"
  # The text route and the direct route must give the same code and data.
  "$BIN/llvm-objdump" -d -s -r "$TMP/$name.o" >"$TMP/$name.direct.txt"
  "$BIN/llvm-objdump" -d -s -r "$TMP/$name.mc.o" >"$TMP/$name.text.txt"
  sed -i 's#^.*file format.*$##' "$TMP/$name.direct.txt" "$TMP/$name.text.txt"
  if ! diff -u "$TMP/$name.direct.txt" "$TMP/$name.text.txt" >"$TMP/$name.diff"; then
    echo "MISMATCH between llc -filetype=obj and llc | llvm-mc for $name:" >&2
    head -40 "$TMP/$name.diff" >&2
    fail=1
  fi
  if [[ $CHECK == 1 ]]; then
    cmp -s "$TMP/$name.o" "$RUN/$name.o" || { echo "stale: tests/run/$name.o" >&2; fail=1; }
  else
    cp "$TMP/$name.o" "$RUN/$name.o"
    echo "wrote tests/run/$name.o"
  fi
done

# Boot programs for the linker and packager tests: assembly, one object each.
for s in "$BACKEND_DIR"/tests/boot/*.s; do
  name="$(basename "$s" .s)"
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$s" -o "$TMP/$name.boot.o"
  if [[ $CHECK == 1 ]]; then
    cmp -s "$TMP/$name.boot.o" "$BACKEND_DIR/tests/boot/$name.o" || { echo "stale: tests/boot/$name.o" >&2; fail=1; }
  else
    cp "$TMP/$name.boot.o" "$BACKEND_DIR/tests/boot/$name.o"
    echo "wrote tests/boot/$name.o"
  fi
done

# The platform runtime (crt0 and the HAL's machine code) for the Rust smoke programs.
for s in "$BACKEND_DIR"/../system/ibm_pc/platform/runtime/*.s; do
  name="$(basename "$s" .s)"
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$s" -o "$TMP/$name.plat.o"
  dst="$(dirname "$s")/$name.o"
  if [[ $CHECK == 1 ]]; then
    cmp -s "$TMP/$name.plat.o" "$dst" || { echo "stale: ${dst#"$BACKEND_DIR/../"}" >&2; fail=1; }
  else
    cp "$TMP/$name.plat.o" "$dst"
    echo "wrote ${dst#"$BACKEND_DIR/../"}"
  fi
done
exit $fail
