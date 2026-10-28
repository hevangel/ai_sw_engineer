#!/usr/bin/env bash
# Run the host build interactively: stdin -> core -> stdout.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
cd "$TITLE_DIR"
exec cargo run --release --quiet --bin "$SOFTWARE"
