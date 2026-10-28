#!/usr/bin/env bash
# Record the DOS 1.00 screen transcript from the hardware project's IBM PC RTL.
#
# Runs scripts/rtl_oracle.py inside the project's Docker image against the RTL
# system build (`system/ibm_pc/work/obj/Vibm_pc`, which `system/ibm_pc/scripts/build.sh`
# in ai_hw_engineer produces) and writes oracle/transcripts/dos100_session.txt.
#
# Usage: scripts/rtl_oracle.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$HERE/../../.." && pwd)"
HW="${AI_HW_ENGINEER:-$ROOT/../ai_hw_engineer}"
HW="$(cd "$HW" && pwd)"
IMAGE="${DOCKER_IMAGE:-ai-hw-engineer:latest}"
SIM="$HW/system/ibm_pc/work/obj/Vibm_pc"

[[ -f $SIM ]] || { echo "ERROR: RTL system build not found: $SIM" >&2; exit 1; }
mkdir -p "$HERE/oracle/transcripts"

echo "==> recording the RTL's DOS 1.00 session in $IMAGE"
MSYS_NO_PATHCONV=1 docker run --rm -v "$HW:/hw:ro" -v "$HERE:/emu" "$IMAGE" \
    sh -euc 'cd /tmp && python3 /emu/scripts/rtl_oracle.py /hw/system/ibm_pc/work/obj/Vibm_pc /hw/system/ibm_pc/work/reference /emu/oracle/transcripts/dos100_session.txt'
echo "==> wrote oracle/transcripts/dos100_session.txt"
