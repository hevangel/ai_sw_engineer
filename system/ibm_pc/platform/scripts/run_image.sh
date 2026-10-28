#!/usr/bin/env bash
# Run a target image on a ibm_pc simulator, feeding one oracle transcript's inputs.
# Usage: run_image.sh <primary|secondary|emulator> <image.bin> <input_transcript.txt>
# Writes the observed outputs to stdout in transcript format ("< hex bytes").
#
# primary   -> RTL/ISS in $AI_HW_ENGINEER (see ../README.md for the build used)
# secondary -> third-party emulator under external/
# emulator  -> this system's ../../emulator CLI (host Rust system emulator)
set -euo pipefail

sim="${1:?sim}"; image="${2:?image}"; transcript="${3:?transcript}"
echo "ERROR: run_image.sh for ibm_pc is not implemented yet (sim=$sim image=$image transcript=$transcript)" >&2
exit 2
