#!/usr/bin/env bash
# Build every image format of the IBM PC 5150 profile from source and check the sizes:
#
#   docker run --rm -v $PWD:/workspace -v ai-sw-llvm-build:/build -w /workspace \
#     ai-sw-engineer:latest system/ibm_pc/toolchain/scripts/build_profile.sh
#
# Output in system/ibm_pc/toolchain/build (not committed): the objects, the linked ELF files with
# their maps, the images, `images.json` (what scripts/check_images.py checks) and `artifacts.sha256`.
#
#   hello_rs, irq_rs  Rust, bootable 160 KiB floppy (link.ld, crt0.s)
#   hello.com         Rust, DOS .COM (link_com.ld, crt0_com.s)
#   hellox.exe        the same program as a tiny-model .EXE
#   segx.exe          IR, a two-segment .EXE with segment relocations (link_exe_seg.ld)
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT"
OUT="${OUT:-system/ibm_pc/toolchain/build}"
PLAT=system/ibm_pc/platform
BE=llvm-i8086
BIN="$ROOT/$BE/install/bin"
rm -rf "$OUT" && mkdir -p "$OUT/obj" "$OUT/rt"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR

echo "==> runtime: assemble $PLAT/runtime/*.s"
for s in crt0 crt0_com dos hal; do
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/$s.s" -o "$OUT/rt/$s.o"
done

echo "==> Rust programs (rustc i8086-unknown-none) and the IR program"
for crate in hello_rs irq_rs hello_com; do
  "$BE/scripts/build_rust.sh" "$BE/tests/smoke/$crate" "$crate" "$OUT/obj/$crate" >/dev/null 2>&1
  cp "$OUT/obj/$crate/$crate.o" "$OUT/obj/$crate.o"
done
"$BIN/llc" -mtriple=i8086 -O2 -filetype=obj "$BE/tests/smoke_ir/seg_exe.ll" -o "$OUT/obj/seg_exe.o"
RT=("$ROOT"/$BE/rt/obj/*.o)

link() { # <name> <linker script> <objects...> -- <extra ld args>
  local name=$1 script=$2; shift 2
  local objs=() extra=()
  while [[ $# -gt 0 && $1 != -- ]]; do objs+=("$1"); shift; done
  [[ ${1:-} == -- ]] && { shift; extra=("$@"); }
  "$LD" -T "$script" -o "$OUT/$name.elf" -Map "$OUT/$name.map" "${extra[@]}" "${objs[@]}" 2>&1 | sed "s/^/    $name: /"
}

echo "==> link"
link hello_rs "$PLAT/link.ld" "$OUT/rt/crt0.o" "$OUT/rt/hal.o" "$OUT/obj/hello_rs.o" "${RT[@]}" -- --bin "$OUT/hello_rs.bin" --floppy "$OUT/hello_rs.img" --sign
link irq_rs   "$PLAT/link.ld" "$OUT/rt/crt0.o" "$OUT/rt/hal.o" "$OUT/obj/irq_rs.o"   "${RT[@]}" -- --bin "$OUT/irq_rs.bin"   --floppy "$OUT/irq_rs.img"   --sign
link hello    "$PLAT/link_com.ld" "$OUT/rt/crt0_com.o" "$OUT/rt/dos.o" "$OUT/obj/hello_com.o" "${RT[@]}" -- --com "$OUT/hello.com" --exe "$OUT/hellox.exe"
link segx     "$PLAT/link_exe_seg.ld" "$OUT/obj/seg_exe.o" -- --exe "$OUT/segx.exe"

echo "==> sizes (the linker already refuses a region overflow)"
sz() { printf '    %-12s %8d bytes\n' "$1" "$(wc -c < "$OUT/$1")"; }
for f in hello_rs.img irq_rs.img hello.com hellox.exe segx.exe; do sz "$f"; done
[[ $(wc -c < "$OUT/hello_rs.img") -le 163840 && $(wc -c < "$OUT/irq_rs.img") -le 163840 ]] || { echo "ERROR: a floppy image is larger than 160 KiB" >&2; exit 1; }
[[ $(wc -c < "$OUT/hello.com") -le 65024 ]] || { echo "ERROR: hello.com does not fit one segment" >&2; exit 1; }

cat > "$OUT/images.json" <<JSON
[
 {"name": "hello_rs", "kind": "floppy", "elf": "hello_rs.elf", "map": "hello_rs.map", "image": "hello_rs.img",
  "objects": ["rt/crt0.o", "rt/hal.o", "obj/hello_rs.o", "@rt"], "entry": 31744, "window": [1280, 65536]},
 {"name": "irq_rs", "kind": "floppy", "elf": "irq_rs.elf", "map": "irq_rs.map", "image": "irq_rs.img",
  "objects": ["rt/crt0.o", "rt/hal.o", "obj/irq_rs.o", "@rt"], "entry": 31744, "window": [1280, 65536]},
 {"name": "hello.com", "kind": "com", "elf": "hello.elf", "map": "hello.map", "image": "hello.com",
  "objects": ["rt/crt0_com.o", "rt/dos.o", "obj/hello_com.o", "@rt"], "entry": 256, "window": [256, 65536]},
 {"name": "hellox.exe", "kind": "exe", "elf": "hello.elf", "map": "hello.map", "image": "hellox.exe",
  "objects": ["rt/crt0_com.o", "rt/dos.o", "obj/hello_com.o", "@rt"], "entry": 256, "window": [256, 65536]},
 {"name": "segx.exe", "kind": "exe", "elf": "segx.elf", "map": "segx.map", "image": "segx.exe",
  "objects": ["obj/seg_exe.o"], "entry": null, "window": [256, 1048576], "segment_regions": ["far"]}
]
JSON
(cd "$OUT" && sha256sum hello_rs.img irq_rs.img hello.com hellox.exe segx.exe hello_rs.bin irq_rs.bin > artifacts.sha256)
echo "==> $OUT/artifacts.sha256"; cat "$OUT/artifacts.sha256"
