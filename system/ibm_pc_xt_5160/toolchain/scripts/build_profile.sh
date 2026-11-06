#!/usr/bin/env bash
# Build every image of the IBM PC XT 5160 compiler profile from source and check the sizes
# (issue #59). The XT has the 5150's CPU, BIOS interface and boot path, so the profile reuses the
# 5150 platform unchanged (system/ibm_pc/platform: crt0, crt0_com, dos, hal, the three linker
# scripts) and the shared llvm-i8086 toolchain; spec/profile.md says what differs.
#
#   system/ibm_pc_xt_5160/toolchain/scripts/build_profile.sh
#
# Needs llvm-i8086/install/bin (llvm-i8086/scripts/build_llvm.sh) and the pinned nightly with
# rust-src (the msp430 route of llvm-i8086/rust/README.md), or $I8086_RUSTC.
# Output in system/ibm_pc_xt_5160/toolchain/build (not committed):
#
#   xt_info.img        Rust (smoke/xt_info_rs), bootable 160 KiB floppy: BIOS data area, RAM, ROM sum
#   hello_rs, irq_rs   Rust (llvm-i8086/tests/smoke), bootable 160 KiB floppies
#   hello.com          Rust, PC DOS .COM;  hellox.exe  the same as a tiny-model .EXE
#   segx.exe           IR, a two-segment .EXE with segment relocations
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT"
HERE=system/ibm_pc_xt_5160/toolchain
OUT="${OUT:-$HERE/build}"
PLAT=system/ibm_pc/platform
BE=llvm-i8086
BIN="$ROOT/$BE/install/bin"
[[ -x $BIN/llc ]] || { echo "ERROR: $BIN/llc missing: run llvm-i8086/scripts/build_llvm.sh" >&2; exit 1; }
rm -rf "$OUT" && mkdir -p "$OUT/obj" "$OUT/rt"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR

echo "==> runtime: assemble $PLAT/runtime/*.s (the 5150's, unchanged)"
for s in crt0 crt0_com dos hal; do
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/$s.s" -o "$OUT/rt/$s.o"
done

echo "==> Rust programs and the IR program"
for c in "$HERE/smoke/xt_info_rs xt_info_rs" "$BE/tests/smoke/hello_rs hello_rs" \
         "$BE/tests/smoke/irq_rs irq_rs" "$BE/tests/smoke/hello_com hello_com"; do
  set -- $c
  bash "$BE/scripts/build_rust.sh" "$1" "$2" "$OUT/obj/$2" >"$OUT/obj/$2.log" 2>&1 \
    || { cat "$OUT/obj/$2.log" >&2; exit 1; }
  cp "$OUT/obj/$2/$2.o" "$OUT/obj/$2.o"
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
for p in xt_info hello_rs irq_rs; do
  o=$p; [[ $p == xt_info ]] && o=xt_info_rs
  link "$p" "$PLAT/link.ld" "$OUT/rt/crt0.o" "$OUT/rt/hal.o" "$OUT/obj/$o.o" "${RT[@]}" \
    -- --bin "$OUT/$p.bin" --floppy "$OUT/$p.img" --sign
done
link hello "$PLAT/link_com.ld" "$OUT/rt/crt0_com.o" "$OUT/rt/dos.o" "$OUT/obj/hello_com.o" "${RT[@]}" -- --com "$OUT/hello.com" --exe "$OUT/hellox.exe"
link segx  "$PLAT/link_exe_seg.ld" "$OUT/obj/seg_exe.o" -- --exe "$OUT/segx.exe"

echo "==> sizes (the linker already refuses a region overflow)"
for f in xt_info.img hello_rs.img irq_rs.img hello.com hellox.exe segx.exe; do
  printf '    %-12s %8d bytes\n' "$f" "$(wc -c < "$OUT/$f")"
done
for f in xt_info hello_rs irq_rs; do
  [[ $(wc -c < "$OUT/$f.img") -eq 163840 ]] || { echo "ERROR: $f.img is not a 160 KiB floppy" >&2; exit 1; }
done
[[ $(wc -c < "$OUT/hello.com") -le 65024 ]] || { echo "ERROR: hello.com does not fit one segment" >&2; exit 1; }

floppy() { # <name> <object>
  printf ' {"name": "%s", "kind": "floppy", "floppy_kib": 160, "elf": "%s.elf", "map": "%s.map", "image": "%s.img",
  "objects": ["rt/crt0.o", "rt/hal.o", "obj/%s.o", "@rt"], "entry": 31744, "window": [1280, 65536]},\n' "$1" "$1" "$1" "$1" "$2"
}
{
  echo "["
  floppy xt_info xt_info_rs; floppy hello_rs hello_rs; floppy irq_rs irq_rs
  cat <<'JSON'
 {"name": "hello.com", "kind": "com", "elf": "hello.elf", "map": "hello.map", "image": "hello.com",
  "objects": ["rt/crt0_com.o", "rt/dos.o", "obj/hello_com.o", "@rt"], "entry": 256, "window": [256, 65536]},
 {"name": "hellox.exe", "kind": "exe", "elf": "hello.elf", "map": "hello.map", "image": "hellox.exe",
  "objects": ["rt/crt0_com.o", "rt/dos.o", "obj/hello_com.o", "@rt"], "entry": 256, "window": [256, 65536]},
 {"name": "segx.exe", "kind": "exe", "elf": "segx.elf", "map": "segx.map", "image": "segx.exe",
  "objects": ["obj/seg_exe.o"], "entry": null, "window": [256, 1048576], "segment_regions": ["far"]}
]
JSON
} > "$OUT/images.json"
(cd "$OUT" && sha256sum xt_info.img hello_rs.img irq_rs.img hello.com hellox.exe segx.exe > artifacts.sha256)
echo "==> $OUT/artifacts.sha256"; cat "$OUT/artifacts.sha256"
