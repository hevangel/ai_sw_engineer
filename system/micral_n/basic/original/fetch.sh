#!/usr/bin/env bash
# Fetch SCELBAL (Mark G. Arnold and Nat Wadsworth, 1974-75) as assembled by
# Thomas E. Jones's AS8 from Steve Loboyko's 2001 transcription: the source
# and its listing, pinned to a commit of hansake/Software_for_Intel_8008.
# They stay here, gitignored (MGA's notice permits educational, historical,
# non-commercial use); provenance.md records the URLs and SHA-256.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
base="https://raw.githubusercontent.com/hansake/Software_for_Intel_8008/df573a420605c065ae76639d434d6bc88d22e4e5/scelbal_scelbi"
for f in sc1.asm sc1.lst; do
    want="$(cut -d' ' -f1 "$here/$f.sha256")"
    curl -fsSL "$base/$f" -o "$here/$f"
    got="$(sha256sum "$here/$f" | cut -d' ' -f1)"
    [[ $got == "$want" ]] || { echo "ERROR: $f SHA-256 $got, expected $want" >&2; exit 1; }
    echo "==> $here/$f (SHA-256 ok)"
done
