#!/usr/bin/env bash
# Run SCELBAL on the host: stdin -> teletype -> stdout. Start with SCR.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
cd "$TITLE_DIR"
exec cargo run --release --quiet --bin "$SOFTWARE"
