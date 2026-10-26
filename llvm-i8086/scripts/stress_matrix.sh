#!/usr/bin/env bash
# Build tests/smoke/stress_rs at several optimisation levels and llc
# settings, into <out_dir>/*.o, for the emulator test
#   I8086_STRESS_OBJECTS=<out_dir> cargo test --release --test i8086_rust \
#       -- --ignored the_stress_program_agrees_at_every_optimisation_level
#
#   stress_matrix.sh <out_dir>
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
backend="$(cd "$here/.." && pwd)"
out="${1:?out_dir}"
mkdir -p "$out"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for opt in 1 2 3 s z; do
  case $opt in [0-3]) q="";; *) q="\"";; esac
  for llc in -O2 -O0; do
    name="opt${opt}_llc${llc#-}"
    # build_rust.sh passes -O2 itself; the extra flag overrides it (last wins)
    LLC_FLAGS="$llc" "$here/build_rust.sh" "$backend/tests/smoke/stress_rs" stress_rs "$tmp/$name" \
      --config "profile.release.opt-level=$q$opt$q" >/dev/null 2>"$tmp/$name.log" \
      || { echo "FAILED to build $name:"; tail -5 "$tmp/$name.log"; continue; }
    cp "$tmp/$name/stress_rs.o" "$out/$name.o"
    echo "built $name"
  done
done
