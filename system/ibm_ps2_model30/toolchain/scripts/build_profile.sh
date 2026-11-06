#!/usr/bin/env bash
# Build every image of the IBM PS/2 Model 30 compiler profile from source and check the sizes
# (issue #60). The Model 30 has an 8086 and the PC BIOS interface, so the profile reuses the shared
# llvm-i8086 toolchain and, from the 5150 platform, hal.s, crt0_com.s, dos.s and the DOS linker
# scripts. Its own (system/ibm_ps2_model30/platform): crt0.s and link.ld for the 720 KiB diskette,
# ps2.s (INT 10h function 1Ah). spec/profile.md says what differs.
#
#   system/ibm_ps2_model30/toolchain/scripts/build_profile.sh
#
# Needs llvm-i8086/install/bin (llvm-i8086/scripts/build_llvm.sh) and the pinned nightly with
# rust-src (the msp430 route of llvm-i8086/rust/README.md), or $I8086_RUSTC.
# Output in system/ibm_ps2_model30/toolchain/build (not committed):
#
#   ps2_info.img       Rust (smoke/ps2_info_rs), bootable 720 KiB diskette: BIOS data area, MCGA,
#                      model byte and date, RAM, ROM sum
#   hello_rs, irq_rs   Rust (llvm-i8086/tests/smoke), bootable 720 KiB diskettes
#   hello.com          Rust, PC DOS .COM;  hellox.exe  the same as a tiny-model .EXE
#   segx.exe           IR, a two-segment .EXE with segment relocations
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
cd "$ROOT"
HERE=system/ibm_ps2_model30/toolchain
OUT="${OUT:-$HERE/build}"
PLAT=system/ibm_pc/platform
PS2=system/ibm_ps2_model30/platform
BE=llvm-i8086
BIN="$ROOT/$BE/install/bin"
[[ -x $BIN/llc ]] || { echo "ERROR: $BIN/llc missing: run llvm-i8086/scripts/build_llvm.sh" >&2; exit 1; }
rm -rf "$OUT" && mkdir -p "$OUT/obj" "$OUT/rt"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR_LD:-/tmp/i8086-ld-target}"
cargo +1.99.1 build --quiet --release --manifest-path "$BE/asm/Cargo.toml"
LD="$CARGO_TARGET_DIR/release/i8086-ld"
unset CARGO_TARGET_DIR

echo "==> runtime: the PS/2 crt0 and ps2.s, and the 5150's crt0_com, dos and hal"
for s in crt0 ps2; do
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$PS2/runtime/$s.s" -o "$OUT/rt/$s.o"
done
for s in crt0_com dos hal; do
  "$BIN/llvm-mc" -triple=i8086 -filetype=obj "$PLAT/runtime/$s.s" -o "$OUT/rt/$s.o"
done

echo "==> Rust programs and the IR program"
for c in "$HERE/smoke/ps2_info_rs ps2_info_rs" "$BE/tests/smoke/hello_rs hello_rs" \
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
for p in ps2_info hello_rs irq_rs; do
  o=$p; extra=(); [[ $p == ps2_info ]] && { o=ps2_info_rs; extra=("$OUT/rt/ps2.o"); }
  link "$p" "$PS2/link.ld" "$OUT/rt/crt0.o" "$OUT/rt/hal.o" "${extra[@]}" "$OUT/obj/$o.o" "${RT[@]}" \
    -- --bin "$OUT/$p.bin" --floppy "$OUT/$p.img" --floppy-kib 720 --sign
done
link hello "$PLAT/link_com.ld" "$OUT/rt/crt0_com.o" "$OUT/rt/dos.o" "$OUT/obj/hello_com.o" "${RT[@]}" -- --com "$OUT/hello.com" --exe "$OUT/hellox.exe"
link segx  "$PLAT/link_exe_seg.ld" "$OUT/obj/seg_exe.o" -- --exe "$OUT/segx.exe"

echo "==> sizes (the linker already refuses a region overflow)"
for f in ps2_info.img hello_rs.img irq_rs.img hello.com hellox.exe segx.exe; do
  printf '    %-12s %8d bytes\n' "$f" "$(wc -c < "$OUT/$f")"
done
for f in ps2_info hello_rs irq_rs; do
  [[ $(wc -c < "$OUT/$f.img") -eq 737280 ]] || { echo "ERROR: $f.img is not a 720 KiB diskette" >&2; exit 1; }
done
[[ $(wc -c < "$OUT/hello.com") -le 65024 ]] || { echo "ERROR: hello.com does not fit one segment" >&2; exit 1; }

floppy() { # <name> <object> [<extra object>]
  local more=""; [[ -n ${3:-} ]] && more="\"$3\", "
  printf ' {"name": "%s", "kind": "floppy", "floppy_kib": 720, "elf": "%s.elf", "map": "%s.map", "image": "%s.img",
  "objects": ["rt/crt0.o", "rt/hal.o", %s"obj/%s.o", "@rt"], "entry": 31744, "window": [1280, 65536]},\n' "$1" "$1" "$1" "$1" "$more" "$2"
}
{
  echo "["
  floppy ps2_info ps2_info_rs rt/ps2.o; floppy hello_rs hello_rs; floppy irq_rs irq_rs
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
(cd "$OUT" && sha256sum ps2_info.img hello_rs.img irq_rs.img hello.com hellox.exe segx.exe > artifacts.sha256)
echo "==> $OUT/artifacts.sha256"; cat "$OUT/artifacts.sha256"
