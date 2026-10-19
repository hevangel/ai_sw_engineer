#!/usr/bin/env bash
# Compare a produced busicom_141pf port-level trace against the committed oracle
# (firmware/oracle/rom_port_trace.txt), EXACTLY -- every payload line must match
# in order. Payload lines are the trace events (#EXAMPLE / #RESET / #STEP /
# LAMPS / ROW); the oracle's comment header (lines beginning "# ") is ignored on
# both sides so provenance comments never affect the verdict.
#
# Usage: compare.sh <produced_trace.txt> [oracle_trace.txt]
# Exit 0 iff identical; prints a unified diff and exits 1 otherwise.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
title_dir="$(cd "$here/../../firmware" && pwd)"

got="${1:?produced_trace.txt}"
oracle="${2:-$title_dir/oracle/rom_port_trace.txt}"

[[ -f $got ]] || { echo "ERROR: produced trace not found: $got" >&2; exit 2; }
[[ -f $oracle ]] || { echo "ERROR: oracle not found: $oracle" >&2; exit 2; }

payload() { grep -E '^(#EXAMPLE|#RESET|#STEP|LAMPS|ROW)' "$1"; }

o=$(mktemp); g=$(mktemp)
trap 'rm -f "$o" "$g"' EXIT
payload "$oracle" > "$o"
payload "$got" > "$g"

oe=$(grep -c '^#EXAMPLE' "$o" || true)
ge=$(grep -c '^#EXAMPLE' "$g" || true)

if diff -u "$o" "$g" > /tmp/_busicom_cmp.diff 2>&1; then
    echo "PASS: trace identical to oracle ($ge/$oe examples, $(wc -l < "$g") payload lines)"
    exit 0
else
    echo "FAIL: trace differs from oracle (oracle $oe examples, produced $ge examples)"
    head -60 /tmp/_busicom_cmp.diff
    exit 1
fi
