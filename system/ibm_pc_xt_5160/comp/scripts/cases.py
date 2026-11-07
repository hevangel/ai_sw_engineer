#!/usr/bin/env python3
"""The COMP test cases: input files (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

COMP ends by asking "Compare more files (Y/N)?"; every case answers N. The inputs are this
repository's own bytes."""
import sys
from pathlib import Path

TEXT = b"The quick brown fox\r\njumps over the lazy dog\r\n\x1a"


def flip(data, *offsets):
    b = bytearray(data)
    for o in offsets:
        b[o] ^= 0x20
    return bytes(b)


FILES = {
    "A.TXT": TEXT,
    "SAME.TXT": TEXT,
    "THREE.TXT": flip(TEXT, 4, 10, 30),
    "MANY.TXT": flip(TEXT, *range(0, 24)),
    "SHORT.TXT": TEXT[:20],
    "BIN.DAT": bytes(range(64)),
    "BIN2.DAT": bytes(range(64)),
    "X1.DAT": b"abc\x1a", "X2.DAT": b"abc\x1a", "X1.BAK": b"abc\x1a", "X2.BAK": b"abd\x1a",
}
MORE = "Compare more files"
CASES = [
    ("p01_same", "A.TXT,SAME.TXT", "comp a.txt same.txt", [(MORE, "n")]),
    ("p02_three", "A.TXT,THREE.TXT", "comp a.txt three.txt", [(MORE, "n")]),
    ("p03_ten", "A.TXT,MANY.TXT", "comp a.txt many.txt", [(MORE, "n")]),
    ("p04_sizes", "A.TXT,SHORT.TXT", "comp a.txt short.txt", [(MORE, "n")]),
    ("p05_missing", "A.TXT", "comp a.txt nofile.txt", [(MORE, "n")]),
    ("p06_no_eof", "BIN.DAT,BIN2.DAT", "comp bin.dat bin2.dat", [(MORE, "n")]),
    ("p07_prompts", "A.TXT,SAME.TXT", "comp", [("Enter primary", "a.txt\r"), ("Enter 2nd", "same.txt\r"), (MORE, "n")]),
    ("p08_wildcards", "X1.DAT,X2.DAT,X1.BAK,X2.BAK", "comp *.dat *.bak", [(MORE, "n")]),
    ("p09_drive_only", "A.TXT", "comp a.txt a:", [(MORE, "n")]),
    ("p10_missing_first", "A.TXT", "comp nofile.txt a.txt", [(MORE, "n")]),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, data in FILES.items():
        (out / name).write_bytes(data)
    for name, files, cmd, pairs in CASES:
        print("\t".join([name, files, cmd] + [x for p in pairs for x in p]))


if __name__ == "__main__":
    main()
