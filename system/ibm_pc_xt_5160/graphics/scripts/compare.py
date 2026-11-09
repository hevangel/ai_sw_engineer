#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir>

Compares, case by case, by the rule of plans/testplan.md:
* the screen: all 25 rows, right-trimmed (every case ends with DRAW setting mode 3, which
  clears the screen, so nothing of the boot is left on it);
* the printed bytes: the `# printed:` line (OUT.PRN's length and SHA-256, scripts/capture.sh).
Transcripts are split on `\\n` only. Exit 0 if every case of the expected directory is
identical in the run."""
import sys
from pathlib import Path


def read(path):
    text = path.read_text(encoding="latin-1").split("\n")
    rows = [l[1:].rstrip() for l in text if l.startswith("|")]
    printed = next((l for l in text if l.startswith("# printed: ")), None)
    return rows, printed


want_dir, got_dir = Path(sys.argv[1]), Path(sys.argv[2])
bad = 0
cases = sorted(want_dir.glob("*.txt"))
for w in cases:
    g = got_dir / w.name
    if not g.exists():
        print(f"MISSING {w.stem}")
        bad += 1
        continue
    want, want_p = read(w)
    got, got_p = read(g)
    if want != got:
        bad += 1
        print(f"DIFF {w.stem} screen\n  expected {want}\n  got      {got}")
    elif want_p is None or want_p != got_p:
        bad += 1
        print(f"DIFF {w.stem} printed\n  expected {want_p}\n  got      {got_p}")
    else:
        print(f"ok   {w.stem} ({want_p[len('# printed: '):]})")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
