#!/usr/bin/env bash
# Run a DOS command on the RTL (PC DOS 1.00 in drive A) and record a transcript.
#
# Usage: scripts/rtl_oracle.sh <name> <disk_b.img> "<command>"
#   e.g. scripts/rtl_oracle.sh 001_workdisk "$AI_HW_ENGINEER/system/ibm_pc/work/reference/workdisk.img" "chkdsk b:"
# Environment: DOS_IMG   drive A image (default: the reference pcdos100.img)
#              OUT_DIR   where the transcript goes (default: oracle/transcripts)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
[[ $# -eq 3 ]] || fail "usage: $0 <name> <disk_b.img> \"<command>\""
NAME=$1 DISK=$(cd "$(dirname "$2")" && pwd)/$(basename "$2") CMD=$3
HW="$(cd "$AI_HW_ENGINEER" && pwd)"
SIM="$HW/system/ibm_pc/work/obj/Vibm_pc"
[[ -f $SIM ]] || fail "RTL system build not found: $SIM"
DOS="${DOS_IMG:-$HW/system/ibm_pc/work/reference/pcdos100.img}"
OUT="${OUT_DIR:-$TITLE_DIR/oracle/transcripts}"
WORK="$BUILD_DIR/oracle"
mkdir -p "$WORK" "$OUT"
cp "$DISK" "$WORK/$NAME.before.img"
cp "$DOS" "$WORK/$NAME.dos.img"
OUT_REL=$(realpath --relative-to="$ROOT" "$OUT")
status "running '$CMD' on $NAME"
MSYS_NO_PATHCONV=1 docker run --rm -v "$HW:/hw:ro" -v "$ROOT:/repo" "${DOCKER_IMAGE:-ai-hw-engineer:latest}" \
    sh -euc "cd /tmp && cp /repo/system/ibm_pc/$SOFTWARE/build/oracle/$NAME.before.img /tmp/b.img && cp /repo/system/ibm_pc/$SOFTWARE/build/oracle/$NAME.dos.img /tmp/a.img && python3 /repo/system/ibm_pc/$SOFTWARE/scripts/rtl_oracle.py /hw/system/ibm_pc/work/obj/Vibm_pc /hw/system/ibm_pc/work/reference /tmp/a.img /tmp/b.img '$CMD' /repo/$OUT_REL/$NAME.txt /repo/system/ibm_pc/$SOFTWARE/build/oracle/$NAME.after.img"
