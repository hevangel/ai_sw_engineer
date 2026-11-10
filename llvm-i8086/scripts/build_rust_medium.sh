#!/usr/bin/env bash
# A Rust crate -> the objects and linker script of a medium-model (several code segments)
# PC DOS program (spec/abi.md D10, rust/README.md "Medium model").
#
#   build_rust_medium.sh <crate_dir> <bin_name> <out_dir> [extra cargo args...]
#   (env MEDIUM_CAP0: bytes of code for segment 0, default 0x4000; MEDIUM_CAP: bytes per far
#    segment, default 0xD000; MEDIUM_SEG0: functions kept in segment 0, comma-separated,
#    default main; MEDIUM_NEAR: declared functions that stay near; plus build_rust.sh's env)
#
# 1. build_rust.sh makes the crate's whole-program IR (<bin>.ll);
# 2. ir_medium.py --far-intrinsics, llc: a near-model object, only to measure each function;
# 3. ir_medium.py --far-intrinsics --medium: every function far (cc 1000), packed into
#    segments; llc -> <bin>_medium.s;
# 4. seg_rt.py: each far segment's own copy of the runtime routines it calls (rt_s<n>.o);
# 5. medium_ld.py: <bin>.ld for the segments used.
# Outputs in <out_dir>: <bin>_medium.o, rt_s*.o, <bin>.ld, <bin>.segments. Link them with
# rt/medium/start_far.o, rt/medium/far.o, the program's own far assembly and rt/obj/*.o:
#   i8086-ld -T <bin>.ld --exe prog.exe start_far.o far.o ... <bin>_medium.o rt_s*.o rt/obj/*.o
set -euo pipefail
crate="$(cd "${1:?crate_dir}" && pwd)"
bin="${2:?bin_name}"
out="${3:?out_dir}"
shift 3
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
be="$(cd "$here/.." && pwd)"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
LLC="$be/install/bin/llc -mtriple=i8086 -O2 -function-sections -data-sections ${LLC_FLAGS:-}"
MC="$be/install/bin/llvm-mc -triple=i8086 -filetype=obj"

bash "$here/build_rust.sh" "$crate" "$bin" "$out" "$@"
python3 -I "$here/ir_medium.py" "$out/$bin.ll" "$out/${bin}_near.ll" --far-intrinsics
$LLC -filetype=obj "$out/${bin}_near.ll" -o "$out/${bin}_near.o"
"$be/install/bin/llvm-readobj" --sections "$out/${bin}_near.o" |
  awk '/Name: /{n=$2} /Size: /{if (n ~ /^\.text\./) print $2, substr(n,7)}' > "$out/${bin}_sizes.txt"
python3 -I "$here/ir_medium.py" "$out/$bin.ll" "$out/${bin}_medium.ll" --far-intrinsics \
  --medium "$out/${bin}_sizes.txt" --cap0 "${MEDIUM_CAP0:-0x4000}" --cap "${MEDIUM_CAP:-0xD000}" \
  --seg0 "${MEDIUM_SEG0:-main}" --near "${MEDIUM_NEAR:-}"
mv "$out/${bin}_medium.ll.segments" "$out/$bin.segments"
$LLC "$out/${bin}_medium.ll" -o "$out/${bin}_medium.s"
rm -f "$out"/rt_s*.s "$out"/rt_s*.o
$LLC "$be/rt/ir/di.ll" -o "$out/rt_di.s"
python3 -I "$here/seg_rt.py" "$out/${bin}_medium.s" "$out/${bin}_medium2.s" "$out" \
  "$be/rt/asm/divsi.s" "$be/rt/asm/mem.s" "$out/rt_di.s"
for f in "$out"/rt_s*.s; do [[ -e $f ]] && $MC "$f" -o "${f%.s}.o"; done
$MC "$out/${bin}_medium2.s" -o "$out/${bin}_medium.o"
segs=$(python3 -I -c 'import sys; print(max([int(l.split()[1]) for l in open(sys.argv[1]) if not l.startswith("#")] + [1]))' "$out/$bin.segments")
python3 -I "$here/medium_ld.py" "$segs" > "$out/$bin.ld"
echo "==> $out/${bin}_medium.o, $segs far segment(s), $out/$bin.ld"
