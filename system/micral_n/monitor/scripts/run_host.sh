#!/usr/bin/env bash
# Run monitor on the host: stdin -> teletype -> stdout.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
cd "$TITLE_DIR"
exec cargo run --release --quiet --bin "$SOFTWARE"
