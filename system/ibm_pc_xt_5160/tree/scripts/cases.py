#!/usr/bin/env python3
"""The TREE test cases: drive B disks built with pcdisk, and the command typed for each.

    python3 -I scripts/cases.py DIR    # writes the input files to DIR; prints one tab-separated
                                       # line per case: name, setup, command

TREE's output on the 25-file DOS disk would not fit the screen, so each case builds its own
360 KiB disk for drive B (pcdisk blank 360). Setup is a comma-separated list of steps for that
disk: `b:mkdir:PATH`, `b:label:LABEL`, `b:add:PATH=FILE` (FILE written to DIR here).
The inputs are this repository's own bytes."""
import sys
from pathlib import Path

FILES = {"ONE.TXT": b"one\r\n\x1a", "TWO.DAT": bytes(100), "BIG.BIN": bytes(1500)}

NESTED = "b:mkdir:SUB1,b:mkdir:SUB2,b:mkdir:SUB2\\INNER,b:mkdir:SUB2\\INNER\\DEEP"
WITH_FILES = NESTED + ",b:add:ROOT.TXT=ONE.TXT,b:add:SUB1\\A.TXT=ONE.TXT,b:add:SUB1\\B.DAT=TWO.DAT," \
    "b:add:SUB2\\INNER\\C.BIN=BIG.BIN"
CASES = [
    ("t01_none", "b:add:ROOT.TXT=ONE.TXT", "tree b:"),
    ("t02_nested", NESTED, "tree b:"),
    ("t03_files", WITH_FILES, "tree b: /f"),
    ("t04_label", NESTED + ",b:label:TESTDISK", "tree b:"),
    ("t05_label_files", WITH_FILES + ",b:label:MY LABEL", "tree b: /F"),
    ("t06_bad_drive", "b:mkdir:SUB1", "tree q:"),
    ("t07_bad_param", "b:mkdir:SUB1", "tree b: /x"),
    ("t08_empty_dirs", "b:mkdir:A,b:mkdir:B,b:mkdir:C,b:mkdir:A\\X", "tree b:"),
    ("t09_default_drive", "-", "tree"),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, data in FILES.items():
        (out / name).write_bytes(data)
    for name, setup, cmd in CASES:
        print(f"{name}\t{setup}\t{cmd}")


if __name__ == "__main__":
    main()
