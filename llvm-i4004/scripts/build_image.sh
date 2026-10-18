#!/usr/bin/env bash
# Build a target image for one software title.
# Usage: build_image.sh <title_dir> <out_dir> <linker_script>
# Produces <out_dir>/<software>.{ll,s,elf,bin,map}.
#
# Expected steps once the backend exists:
#   cargo +<toolchain> rustc --release --bin <software> --target rust/i4004-none.json \
#       -Zbuild-std=core -- --emit=llvm-ir,asm -C link-arg=-T<linker_script>
#   llvm-objcopy -O binary <elf> <bin>
set -euo pipefail
title_dir="${1:?title_dir}"; out_dir="${2:?out_dir}"; link="${3:?linker_script}"
echo "ERROR: llvm-i4004 build_image.sh not implemented (title=$title_dir out=$out_dir link=$link)" >&2
exit 2
