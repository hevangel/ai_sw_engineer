#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir>

Compares, case by case, the rows chosen by the rule of plans/testplan.md (empty rows included):
if the expected transcript shows the command line, the rows from the one after it to the next
A> prompt, in both; otherwise (the output scrolled it off the screen) the expected rows from the
top through its final A> against the same number of the run's rows ending with its final A>.
Exit 0 if every case of the expected directory is identical in the run."""
import sys
from pathlib import Path


def screen(path):
    return [l[1:].rstrip() for l in path.read_text(encoding="utf-8", errors="replace").splitlines() if l.startswith("|")]


def is_command(r):
    return r.startswith("A>") and len(r) > 2


def compared(want, got):
    c = next((i for i, x in enumerate(want) if is_command(x)), None)
    if c is not None:
        end = next(i for i in range(c + 1, len(want)) if want[i] == "A>")
        s = max((i for i, x in enumerate(got) if is_command(x)), default=None)
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
    a, b = compared(screen(w), screen(g))
    if a == b:
        print(f"ok   {w.stem}")
    else:
        bad += 1
        print(f"DIFF {w.stem}\n  expected {a}\n  got      {b}")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
