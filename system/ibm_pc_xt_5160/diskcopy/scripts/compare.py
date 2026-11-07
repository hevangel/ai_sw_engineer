#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir> [<run work dir> <original drive A dir>]

Compares, case by case, the rows chosen by the rule of plans/testplan.md (empty rows included):
if the expected transcript shows the case's command line (`A>` + the command of its
`# oracle:` header), the rows from the one after it to the next A> prompt, in both (the run's
last such command line); otherwise (the output scrolled it off the screen, as in most cases)
the expected rows from the top through its final A> against the same number of the run's rows
ending with its final A>.

With the run's work directory (capture_dos.sh's WORK: <case>.img, drive A as the run had it,
and <case>_b_after.img, drive B as the run left it) and a directory of the original's drive A
per case (<case>.img: PC DOS 2.00 disk 1 and the case's files), it also compares drive B with
the expected `# drive B after: sha256` by the drive B rule of the test plan: for a case that
copies drive A, every sector of drive B that holds the copy of the run's drive A sector (which
has the program under test as DISKCOPY.COM) counts as the copy of the original's sector.
Exit 0 if every case of the expected directory is identical in the run."""
import hashlib
import sys
from pathlib import Path

GEOMETRY = {163840: (1, 8), 184320: (1, 9), 327680: (2, 8), 368640: (2, 9)}


def read(path):
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    cmd = next(l[len("# oracle: "):] for l in text if l.startswith("# oracle: "))
    b = next((l.split()[-1] for l in text if l.startswith("# drive B after: sha256 ")), None)
    return cmd, [l[1:].rstrip() for l in text if l.startswith("|")], b


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


def copies_drive_a(cmd):
    parts = cmd.split()
    return not (len(parts) > 1 and parts[1].lower().startswith("b:"))


def normalized(after, run_a, orig_a):
    out = bytearray(after)
    if run_a == orig_a:
        return bytes(out)
    sh, ss = GEOMETRY[len(run_a)]
    th, ts = GEOMETRY[len(after)]
    for c in range(40):
        for h in range(min(sh, th)):
            for r in range(min(ss, ts)):
                s = ((c * sh + h) * ss + r) * 512
                t = ((c * th + h) * ts + r) * 512
                if after[t:t + 512] == run_a[s:s + 512]:
                    out[t:t + 512] = orig_a[s:s + 512]
    return bytes(out)


def drive_b(name, cmd, work, orig):
    after = work / f"{name}_b_after.img"
    if not after.exists():
        return None
    data = after.read_bytes()
    if copies_drive_a(cmd):
        data = normalized(data, (work / f"{name}.img").read_bytes(), (orig / f"{name}.img").read_bytes())
    return hashlib.sha256(data).hexdigest()


want_dir, got_dir = Path(sys.argv[1]), Path(sys.argv[2])
work, orig = (Path(sys.argv[3]), Path(sys.argv[4])) if len(sys.argv) > 4 else (None, None)
bad = 0
cases = sorted(want_dir.glob("*.txt"))
for w in cases:
    g = got_dir / w.name
    if not g.exists():
        print(f"MISSING {w.stem}")
        bad += 1
        continue
    cmd, want, want_b = read(w)
    a, b = compared(want, read(g)[1], cmd)
    problems = []
    if a != b:
        problems.append(f"  expected {a}\n  got      {b}")
    if want_b and work:
        got_b = drive_b(w.stem, cmd, work, orig)
        if got_b != want_b:
            problems.append(f"  drive B: expected {want_b}\n           got      {got_b}")
    if problems:
        bad += 1
        print(f"DIFF {w.stem}\n" + "\n".join(problems))
    else:
        print(f"ok   {w.stem}" + (" (screen, drive B)" if want_b and work else " (screen)"))
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
