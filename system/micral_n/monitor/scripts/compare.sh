#!/usr/bin/env bash
# The target check: run_sim.sh compares against the oracle.
set -euo pipefail
exec "$(dirname "${BASH_SOURCE[0]}")/run_sim.sh"
