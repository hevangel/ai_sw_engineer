#!/usr/bin/env python3
"""The ASSIGN test cases: input files (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

ASSIGN stays resident and redirects drive letters. Each case types ASSIGN commands and then
commands whose output shows the redirection (DIR of a pattern, TYPE), all at the A> prompt. Drive
A is PC DOS 2.00 disk 1 plus ATEXT.TXT; drive B a pcdisk disk holding ONLY.TXT. A wait text
starting with `$` waits for the last non-blank row to end with the rest, once the screen has
changed. The inputs are this repository's own bytes."""
import sys
from pathlib import Path

FILES = {"ATEXT.TXT": b"this is on drive A\r\n\x1a", "ONLY.TXT": b"this is on drive B\r\n\x1a"}
SETUP = "ATEXT.TXT,b:add:ONLY.TXT=ONLY.TXT"


def then(*cmds):
    return [("$A>", c + "\r") for c in cmds]


CASES = [
    ("s01_a_to_b", SETUP, "assign a=b", then("dir a:*.txt", "assign", "dir a:*.txt")),
    ("s02_swap", SETUP, "assign a=b b=a", then("type a:only.txt", "type b:atext.txt", "assign")),
    ("s03_new_letter", SETUP, "assign c=b", then("type c:only.txt", "dir c:*.txt", "assign")),
    ("s04_colons_and_case", SETUP, "assign A:=b:", then("type a:only.txt", "assign")),
    ("s05_reassign", SETUP, "assign a=b", then("assign c=b", "type a:atext.txt", "type c:only.txt", "assign")),
    ("s06_bad_drive", SETUP, "assign a=q", then("type a:atext.txt")),
    ("s07_bad_syntax", SETUP, "assign a", then("assign a=", "assign ab=c", "type a:atext.txt")),
    ("s08_reset_only", SETUP, "assign", then("type a:atext.txt")),
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
