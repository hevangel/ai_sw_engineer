#!/usr/bin/env bash
# Full flow: lint, host replay, LLVM IR, target image, simulator compare.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"
here="$(dirname "${BASH_SOURCE[0]}")"
cd "$TITLE_DIR"

status "fmt + clippy"
cargo fmt --check
cargo clippy --all-targets -- -D warnings

status "host replay against oracle"
cargo test

"$here/build.sh" all

if [[ -f $BUILD_DIR/target/$SOFTWARE.bin ]]; then
    "$here/run_sim.sh"
    "$here/compare.sh"
else
    status "SKIP simulator stage: no target image yet"
fi
status "done"
