#!/usr/bin/env bash
# Compare simulator output (build/sim/) with the oracle transcripts.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/config.sh"

outputs() { grep -E '^[[:space:]]*<' "$1" | sed -E 's/#.*//; s/^[[:space:]]*<//' | tr -s ' \t\r\n' '\n' | sed '/^$/d' | tr 'A-F' 'a-f'; }

pass=0
total=0
for t in "$TITLE_DIR"/oracle/transcripts/*.txt; do
    name=$(basename "$t")
    got="$BUILD_DIR/sim/$name"
    total=$((total + 1))
    if [[ -f $got ]] && diff <(outputs "$t") <(outputs "$got") > /dev/null; then
        pass=$((pass + 1))
        status "PASS $name"
    else
        status "FAIL $name"
    fi
done
status "$pass/$total transcripts identical"
[[ $pass -eq $total ]]
