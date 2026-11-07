#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir>

Compares, case by case, the rows plans/testplan.md names (empty rows included): when the
expected screen shows the command line ("A>" and the command of its "# oracle:" header), the rows
from the one after it to the next "A>"; when the command line has scrolled off (m02-m04,
m07-m10), the expected screen's rows from the top to its last "A>" against as many rows of the
run ending at its last "A>". Exit 0 if every case of the expected directory is identical in the run."""
import sys
from pathlib import Path


def screen(path):
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    cmd = next(l[len("# oracle: "):] for l in text if l.startswith("# oracle: "))
    return cmd, [l[1:].rstrip() for l in text if l.startswith("|")]


def compared(rows, cmd, visible, n):
    line = "A>" + cmd
    if visible:
        if line not in rows:
            return None
        c = rows.index(line)
        ends = [i for i in range(c + 1, len(rows)) if rows[i] == "A>"]
        return rows[c + 1:ends[0] + 1] if ends else None
    ends = [i for i, r in enumerate(rows) if r == "A>"]
    if not ends:
        return None
    return rows[max(0, ends[-1] + 1 - n):ends[-1] + 1]


want_dir, got_dir = Path(sys.argv[1]), Path(sys.argv[2])
bad = 0
cases = sorted(want_dir.glob("*.txt"))
for w in cases:
    g = got_dir / w.name
    if not g.exists():
        print(f"MISSING {w.stem}")
        bad += 1
        continue
    cmd, wrows = screen(w)
    visible = ("A>" + cmd) in wrows
    a = compared(wrows, cmd, visible, len(wrows))
    b = compared(screen(g)[1], cmd, visible, len(a))
    if a == b:
        print(f"ok   {w.stem}")
    else:
        bad += 1
        print(f"DIFF {w.stem}\n  expected {a}\n  got      {b}")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
