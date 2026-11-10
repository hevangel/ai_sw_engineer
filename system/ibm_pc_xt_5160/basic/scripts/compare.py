#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir> [case...]

Compares, case by case, the rows chosen by the rule of plans/testplan.md (empty rows included):
if the expected transcript shows the case's command line (`A>` + the command of its
`# oracle:` header), the rows from the one after it to the next A> prompt, in both; otherwise,
if it ends at DOS (a row that is only `A>`), the expected rows from the top through its last A>
against as many of the run's rows ending with its last A>; otherwise (the case ends in BASIC)
the whole screen. Prints one line per case; exit 0 if every compared case is identical."""
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
    if "A>" in want:
        end = max(i for i, x in enumerate(want) if x == "A>")
        w = want[:end + 1]
        e = max((i for i, x in enumerate(got) if x == "A>"), default=None)
        if e is None:
            return w, got
        return w, got[max(0, e + 1 - len(w)):e + 1]
    return want, got


def main():
    exp, run = Path(sys.argv[1]), Path(sys.argv[2])
    only = set(sys.argv[3:])
    bad = 0
    for p in sorted(run.glob("*.txt")):
        if only and p.stem not in only:
            continue
        cmd, got = read(p)
        _, want = read(exp / p.name)
        w, g = compared(want, got, cmd)
        if w == g:
            print(f"{p.stem}: identical")
        else:
            bad += 1
            print(f"{p.stem}: DIFFERS\n  expected {w}\n  got      {g}")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
