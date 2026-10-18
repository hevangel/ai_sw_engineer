#!/usr/bin/env bash
# Create a new system (platform) from system/_template.
# Usage: ./scripts/new_system.sh <system> <chip>
#   e.g. ./scripts/new_system.sh apple_ii mos6502
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <system> <chip>" >&2
    exit 1
fi
SYSTEM=$1
CHIP=$2
[[ $SYSTEM =~ ^[a-z0-9_]+$ ]] || { echo "ERROR: system name must be [a-z0-9_]+" >&2; exit 1; }
[[ $CHIP =~ ^[a-z0-9_]+$ ]] || { echo "ERROR: chip name must be [a-z0-9_]+" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMPLATE="$ROOT/system/_template"
DEST="$ROOT/system/$SYSTEM"
[[ ! -e $DEST ]] || { echo "ERROR: $DEST already exists" >&2; exit 1; }

echo "==> creating system/$SYSTEM (chip: $CHIP)"
mkdir -p "$DEST"
cp "$TEMPLATE/system_README.md" "$DEST/README.md"
cp -r "$TEMPLATE/platform" "$DEST/platform"

find "$DEST" -type f -print0 | while IFS= read -r -d '' f; do
    sed -i -e "s/\[SYSTEM\]/$SYSTEM/g" -e "s/\[CHIP\]/$CHIP/g" "$f"
done
chmod +x "$DEST"/platform/scripts/*.sh

[[ -d $ROOT/llvm-$CHIP ]] || echo "NOTE: llvm-$CHIP does not exist yet; create it with ./scripts/new_llvm_target.sh $CHIP"
echo "==> done. Next: fill system/$SYSTEM/README.md and platform/memory_map.md, add a row to system/README.md,"
echo "    then ./scripts/new_software.sh $SYSTEM <software>"
