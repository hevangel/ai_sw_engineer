#!/usr/bin/env python3
"""compare.py <expected transcripts dir> <run transcripts dir> [<run disks dir>]

Compares, case by case, the screen rows chosen by the rule of plans/testplan.md (empty rows
included): if the expected transcript shows the case's command line (`A>` + the command of its
`# oracle:` header), the rows from the one after it to the next A> prompt, in both (the run's
last such command line); otherwise the expected rows from the top through its final A> against
the same number of the run's rows ending with its final A>. Then drive B: the `# drive B after:`
lines must be equal; if they are not and the run's disks dir is given (capture_dos.sh's WORK,
holding <case>_b_after.img), the image with the masks of plans/testplan.md (the volume label's
time word; sector 0 for /8 without /S) must have the SHA-256 that ../oracle/drive_b_masked.txt
gives for the case ("masked").
Exit 0 if every case of the expected directory is identical in the run."""
import hashlib
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent


def read(path):
    text = path.read_text(encoding="utf-8", errors="replace").splitlines()
    cmd = next(l[len("# oracle: "):] for l in text if l.startswith("# oracle: "))
    disk = next((l[len("# drive B after: "):] for l in text if l.startswith("# drive B after: ")), None)
    return cmd, [l[1:].rstrip() for l in text if l.startswith("|")], disk


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


def masked(cmd, img):
    m = bytearray(img)
    root = 3 if m[512] in (0xFF, 0xFE) else 5
    for e in range(16):
        at = root * 512 + e * 32
        if m[at] not in (0, 0xE5) and m[at + 11] == 0x08:
            m[at + 22:at + 24] = b"\0\0"
    c = cmd.lower()
    if "/8" in c and "/s" not in c:
        m[:512] = bytes(512)
    return bytes(m)


MASKED = dict(l.split() for l in (HERE / "oracle/drive_b_masked.txt").read_text().splitlines()
              if l and not l.startswith("#"))

want_dir, got_dir = Path(sys.argv[1]), Path(sys.argv[2])
disks = Path(sys.argv[3]) if len(sys.argv) > 3 else None
bad = 0
cases = sorted(want_dir.glob("*.txt"))
for w in cases:
    g = got_dir / w.name
    if not g.exists():
        print(f"MISSING {w.stem}")
        bad += 1
        continue
    cmd, want, wdisk = read(w)
    _, rows, gdisk = read(g)
    a, b = compared(want, rows, cmd)
    disk = "identical"
    if wdisk != gdisk:
        img = disks / f"{w.stem}_b_after.img" if disks else None
        if img and img.exists() and hashlib.sha256(masked(cmd, img.read_bytes())).hexdigest() == MASKED.get(w.stem):
            disk = "masked"
        else:
            disk = None
    if a == b and disk:
        print(f"ok   {w.stem} (drive B {disk})")
    else:
        bad += 1
        print(f"DIFF {w.stem}")
        if a != b:
            print(f"  expected {a}\n  got      {b}")
        if not disk:
            print(f"  drive B expected {wdisk}\n  drive B got      {gdisk}")
print(f"{len(cases) - bad} of {len(cases)} identical")
sys.exit(1 if bad else 0)
