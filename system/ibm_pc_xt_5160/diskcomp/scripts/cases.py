#!/usr/bin/env python3
"""The DISKCOMP test cases: drive A and B disks and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the disks and files to DIR; prints one
                                       # tab-separated line per case: name, setup, command,
                                       # then pairs of (text to wait for, keys to type)

DISKCOMP runs from drive A and compares two disks track by track. Drive A is `a:system360`, the
double-sided 360 KiB copy of PC DOS 2.00 disk 1 that capture_dos.sh builds with pcdisk syscopy
(disk 1 itself is single-sided, and side 1 of a one-sided image is where MartyPC and the
hardware differ; see ../diskcopy/spec/spec.md). `b:copy_a` makes drive B a copy of drive A as
built so far, the program under test included; later `b:` steps change that copy with pcdisk.
`b:image:NAME` makes DIR/NAME, written here, the drive B disk (a raw image of zeros is a
formatted disk of zeros to both emulators, so k05 compares, it does not fail to read). A wait text starting with `$`
waits for the last non-blank row to end with the rest, once the screen has changed. The inputs
are this repository's own bytes."""
import sys
from pathlib import Path

FILES = {
    "ZERO360.IMG": bytes(360 * 1024),
    "EXTRA.TXT": b"a file only the second disk has\r\n\x1a",
    "BIG.BIN": bytes(range(256)) * 40,
}
KEY = ("Strike any key when ready", " ")
NO = ("Compare more", "n")
A = "a:system360"
CASES = [
    ("k01_identical", A + ",b:copy_a", "diskcomp a: b:", [KEY, NO]),
    ("k02_extra_file", A + ",b:copy_a,b:add:EXTRA.TXT=EXTRA.TXT", "diskcomp a: b:", [KEY, NO]),
    ("k03_big_file", A + ",b:copy_a,b:add:BIG.BIN=BIG.BIN", "diskcomp a: b:", [KEY, NO]),
    ("k04_other_disk", A + ",b:mkdir:SUB,b:add:SUB\\EXTRA.TXT=EXTRA.TXT", "diskcomp a: b:", [KEY, NO]),
    ("k05_zero_filled", A + ",b:image:ZERO360.IMG", "diskcomp a: b:", [KEY, NO]),
    ("k06_more", A + ",b:copy_a", "diskcomp a: b:",
     [KEY, ("Compare more", "y"), ("$Strike any key when ready", " "), ("$Compare more diskettes (Y/N)?", "n")]),
    ("k07_one_drive", "b:mkdir:SUB,b:add:SUB\\EXTRA.TXT=EXTRA.TXT", "diskcomp b: b:",
     [("$Strike any key when ready", " ")] * 3 + [NO]),
    ("k08_bad_drive", "-", "diskcomp a: q:", []),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, data in FILES.items():
        (out / name).write_bytes(data)
    for name, setup, cmd, pairs in CASES:
        print("\t".join([name, setup, cmd] + [x for p in pairs for x in p]))


if __name__ == "__main__":
    main()
