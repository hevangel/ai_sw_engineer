#!/usr/bin/env bash
# Create a new LLVM backend folder from llvm-_template.
# Usage: ./scripts/new_llvm_target.sh <chip>      e.g. i4004
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <chip>" >&2
    exit 1
fi
CHIP=$1
[[ $CHIP =~ ^[a-z0-9_]+$ ]] || { echo "ERROR: chip name must be [a-z0-9_]+" >&2; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="$ROOT/llvm-$CHIP"
[[ ! -e $DEST ]] || { echo "ERROR: $DEST already exists" >&2; exit 1; }

echo "==> creating llvm-$CHIP"
cp -r "$ROOT/llvm-_template" "$DEST"
mv "$DEST/rust/[CHIP]-none.json" "$DEST/rust/$CHIP-none.json"
find "$DEST" -type f -print0 | while IFS= read -r -d '' f; do
    sed -i "s/\[CHIP\]/$CHIP/g" "$f"
done
chmod +x "$DEST"/scripts/*.sh

echo "==> done. Next: spec/isa.md, spec/abi.md (data layout + calling convention) before any TableGen."
