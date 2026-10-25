#!/usr/bin/env bash
# Full flow: lint, host replay, target image, RTL replay.
# The first three steps run in the ai-sw-engineer image, the last in the
# ai-hw-engineer image (SKIP_SIM=1 stops before it).
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$here/config.sh"
cd "$TITLE_DIR"

status "fmt + clippy"
cargo fmt --check
cargo clippy --all-targets -- -D warnings

status "host replay against oracle"
cargo test --release

"$here/build.sh" target

if [[ -n ${SKIP_SIM:-} ]]; then
    status "SKIP simulator stage (SKIP_SIM)"
else
    "$here/compare.sh"
fi
status "done"
