#!/usr/bin/env bash
# Compare the target against the oracle: run_sim.sh does the comparison
# (byte for byte, per transcript); this runs both load paths.
set -euo pipefail
exec "$(dirname "${BASH_SOURCE[0]}")/run_sim.sh" both
