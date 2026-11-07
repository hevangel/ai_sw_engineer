#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir>

Compares, case by case, the rows each run shows from the command line to the next A> prompt
(empty rows included). Exit 0 if every case of the expected directory is identical in the run."""
import sys
from pathlib import Path


def rows(path):
    r = [l[1:].rstrip() for l in path.read_text(encoding="utf-8", errors="replace").splitlines() if l.startswith("|")]
    start = next(i for i, x in enumerate(r) if x.startswith("A>") and len(x) > 2)
    end = next(i for i in range(start + 1, len(r)) if r[i] == "A>")
    return r[start:end + 1]


want_dir, got_dir = Path(sys.argv[1]), Path(sys.argv[2])
bad = 0
cases = sorted(want_dir.glob("*.txt"))
for w in cases:
    g = got_dir / w.name
    if not g.exists():
        print(f"MISSING {w.stem}")
        bad += 1
        continue
    a, b = rows(w), rows(g)
    if a == b:
        print(f"ok   {w.stem}")
    else:
        bad += 1
        print(f"DIFF {w.stem}\n  expected {a}\n  got      {b}")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
