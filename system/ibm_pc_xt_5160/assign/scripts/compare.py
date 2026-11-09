#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir>

Compares, case by case, the rows chosen by the rule of plans/testplan.md (empty rows included):
from the case's command line (`A>` + the command of its `# oracle:` header, its first such row)
through the screen's last row that is only the prompt `A>`, in both. Every expected transcript
shows its command line; a run without it is compared whole.
Exit 0 if every case of the expected directory is identical in the run."""
import sys
from pathlib import Path


def read(path):
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    cmd = next(l[len("# oracle: "):] for l in text if l.startswith("# oracle: "))
    return cmd, [l[1:].rstrip() for l in text if l.startswith("|")]


def span(rows, cmd):
    line = "A>" + cmd
    if line not in rows:
        return None
    c = rows.index(line)
    e = max((i for i, x in enumerate(rows) if x == "A>"), default=-1)
    return rows[c:e + 1] if e > c else None


want_dir, got_dir = Path(sys.argv[1]), Path(sys.argv[2])
bad = 0
cases = sorted(want_dir.glob("*.txt"))
for w in cases:
    g = got_dir / w.name
    if not g.exists():
        print(f"MISSING {w.stem}")
        bad += 1
        continue
    cmd, want = read(w)
    a = span(want, cmd)
    got = read(g)[1]
    b = span(got, cmd) or got
    if a is None or a != b:
        bad += 1
        print(f"DIFF {w.stem}\n  expected {a}\n  got      {b}")
    else:
        print(f"ok   {w.stem}")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
