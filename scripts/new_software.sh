#!/usr/bin/env bash
# Create a new software title under an existing system.
# Usage: ./scripts/new_software.sh <system> <software>
#   e.g. ./scripts/new_software.sh apple_ii integer_basic
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <system> <software>" >&2
    exit 1
fi
SYSTEM=$1
SOFTWARE=$2
[[ $SOFTWARE =~ ^[a-z][a-z0-9_]*$ ]] || { echo "ERROR: software name must be [a-z][a-z0-9_]* (it becomes the crate name)" >&2; exit 1; }
[[ $SOFTWARE != platform && $SOFTWARE != software ]] || { echo "ERROR: '$SOFTWARE' is reserved" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SYS_DIR="$ROOT/system/$SYSTEM"
DEST="$SYS_DIR/$SOFTWARE"
[[ -d $SYS_DIR/platform ]] || { echo "ERROR: system/$SYSTEM missing; run ./scripts/new_system.sh $SYSTEM <chip>" >&2; exit 1; }
[[ ! -e $DEST ]] || { echo "ERROR: $DEST already exists" >&2; exit 1; }

# The platform records its chip in platform/target.env (written by new_system.sh).
[[ -f $SYS_DIR/platform/target.env ]] || { echo "ERROR: system/$SYSTEM/platform/target.env missing" >&2; exit 1; }
# shellcheck source=/dev/null
CHIP=$(source "$SYS_DIR/platform/target.env" && echo "${CHIP:-}")
[[ -n $CHIP ]] || { echo "ERROR: CHIP not set in system/$SYSTEM/platform/target.env" >&2; exit 1; }

echo "==> creating system/$SYSTEM/$SOFTWARE (chip: $CHIP)"
cp -r "$ROOT/system/_template/software" "$DEST"
find "$DEST" -type f -print0 | while IFS= read -r -d '' f; do
    sed -i -e "s/\[SYSTEM\]/$SYSTEM/g" -e "s/\[SOFTWARE\]/$SOFTWARE/g" -e "s/\[CHIP\]/$CHIP/g" "$f"
done
chmod +x "$DEST"/scripts/*.sh

echo "==> done. Next:"
echo "    1. original/provenance.md   2. oracle/transcripts/ from the ORIGINAL program"
echo "    3. spec/spec.md, spec/io.md 4. plans/   5. src/lib.rs   6. ./scripts/run_all.sh"
