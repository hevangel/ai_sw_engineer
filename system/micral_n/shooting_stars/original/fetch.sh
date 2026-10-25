#!/usr/bin/env bash
# Fetch the Shooting Stars source (BYTE, May 1976, Willard I. Nico) as
# transcribed for the AS assembler by Jim Loos (MIT-licensed repository).
# It stays here, gitignored; provenance.md records the URL and SHA-256.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
url="https://raw.githubusercontent.com/jim11662418/Intel_8008_Single_Board_Computer/main/applications/shooter.asm"
want="$(cut -d' ' -f1 "$here/shooter.asm.sha256")"
curl -fsSL "$url" -o "$here/shooter.asm"
got="$(sha256sum "$here/shooter.asm" | cut -d' ' -f1)"
[[ $got == "$want" ]] || { echo "ERROR: shooter.asm SHA-256 $got, expected $want" >&2; exit 1; }
echo "==> $here/shooter.asm (SHA-256 ok)"
