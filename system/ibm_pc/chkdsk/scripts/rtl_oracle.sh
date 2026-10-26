#!/usr/bin/env bash
# Run the original CHKDSK on the RTL for one case and record oracle/transcripts/<name>.txt.
#
# Usage: scripts/rtl_oracle.sh <name> <disk_b.img> "<command>"
#   e.g. scripts/rtl_oracle.sh 001_workdisk "$AI_HW_ENGINEER/system/ibm_pc/work/reference/workdisk.img" "chkdsk b:"
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
[[ $# -eq 3 ]] || fail "usage: $0 <name> <disk_b.img> \"<command>\""
NAME=$1 DISK=$(cd "$(dirname "$2")" && pwd)/$(basename "$2") CMD=$3
HW="$(cd "$AI_HW_ENGINEER" && pwd)"
SIM="$HW/system/ibm_pc/work/obj/Vibm_pc"
[[ -f $SIM ]] || fail "RTL system build not found: $SIM"
WORK="$BUILD_DIR/oracle"
mkdir -p "$WORK" "$TITLE_DIR/oracle/transcripts"
cp "$DISK" "$WORK/$NAME.before.img"
status "running '$CMD' on $NAME"
MSYS_NO_PATHCONV=1 docker run --rm -v "$HW:/hw:ro" -v "$TITLE_DIR:/title" "${DOCKER_IMAGE:-ai-hw-engineer:latest}" \
    sh -euc "cd /tmp && cp /title/build/oracle/$NAME.before.img /tmp/b.img && python3 /title/scripts/rtl_oracle.py /hw/system/ibm_pc/work/obj/Vibm_pc /hw/system/ibm_pc/work/reference /hw/system/ibm_pc/work/reference/pcdos100.img /tmp/b.img '$CMD' /title/oracle/transcripts/$NAME.txt /title/build/oracle/$NAME.after.img"
