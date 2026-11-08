#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir>

Compares, case by case, the rows chosen by the rule of plans/testplan.md (empty rows included):
if the expected transcript shows the case's command line (`A>` + the command of its
`# oracle:` header), the rows from the one after it to the next A> prompt, in both (the run's
last such command line); otherwise (the output scrolled it off the screen) the expected rows
from the top through its final A> against the same number of the run's rows ending with its
final A>. DISKCOMP writes nothing, so the screen is all there is to compare.
Exit 0 if every case of the expected directory is identical in the run."""
import sys
from pathlib import Path


def read(path):
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    cmd = next(l[len("# oracle: "):] for l in text if l.startswith("# oracle: "))
    return cmd, [l[1:].rstrip() for l in text if l.startswith("|")]


def compared(want, got, cmd):
    line = "A>" + cmd
    if line in want:
        c = want.index(line)
        end = next(i for i in range(c + 1, len(want)) if want[i] == "A>")
        s = max((i for i, x in enumerate(got) if x == line), default=None)
        if s is None:
            return want[c + 1:end + 1], got
        e = next((i for i in range(s + 1, len(got)) if got[i] == "A>"), len(got) - 1)
        return want[c + 1:end + 1], got[s + 1:e + 1]
    end = max(i for i, x in enumerate(want) if x == "A>")
    w = want[:end + 1]
    e = max((i for i, x in enumerate(got) if x == "A>"), default=None)
    if e is None:
        return w, got
    return w, got[max(0, e + 1 - len(w)):e + 1]


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
    a, b = compared(want, read(g)[1], cmd)
    if a != b:
        bad += 1
        print(f"DIFF {w.stem}\n  expected {a}\n  got      {b}")
    else:
        print(f"ok   {w.stem} (screen)")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
