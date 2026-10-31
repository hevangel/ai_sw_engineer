#!/usr/bin/env bash
# Run a target image on a ibm_pc_xt_5160 simulator, feeding one oracle transcript's inputs.
# Usage: run_image.sh <primary|secondary> <image.bin> <input_transcript.txt>
# Writes the observed outputs to stdout in transcript format ("< hex bytes").
#
# primary   -> RTL/ISS in $AI_HW_ENGINEER (see ../README.md for the build used)
# secondary -> third-party emulator under external/
set -euo pipefail

sim="${1:?sim}"; image="${2:?image}"; transcript="${3:?transcript}"
# shellcheck source=../target.env
. "$(dirname "$0")/../target.env"
case "$sim" in
  primary) simdir="$XT_RTL_DIR";;   # ai_hw_engineer/system/ibm_pc_xt_5160, scripts/run_system.sh
  secondary) simdir="$MARTYPC_DIR";; # external/martypc (submodule; see ../../plans/testplan.md)
  *) echo "ERROR: sim must be primary|secondary, got $sim" >&2; exit 2;;
esac
echo "ERROR: run_image.sh for ibm_pc_xt_5160 is not implemented yet (sim=$sim [$simdir] image=$image transcript=$transcript)" >&2
echo "HINT: oracle bring-up order and transcript format are in ../../plans/testplan.md; per-title run_sim.sh carries the real logic (micral_n precedent)." >&2
exit 2
