#!/usr/bin/env python3
"""The DISKCOPY test cases: drive B target disks and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the disks and files to DIR; prints one
                                       # tab-separated line per case: name, setup, command,
                                       # then pairs of (text to wait for, keys to type)

DISKCOPY runs from drive A (PC DOS 2.00 disk 1, a 180 KiB single-sided disk, plus any files a
case adds) and copies it to the disk in drive B. `b:image:NAME` makes DIR/NAME, written here,
the drive B disk; other `b:` steps are pcdisk's on a blank 360 KiB disk
(scripts/capture_dos.sh). Each capture also records the SHA-256 of drive B after the run
(SAVE_B=1); d05 copies twice and needs MARTY_CYCLES=1500000000 on MartyPC. A wait text starting with `$` waits for the last non-blank row to end with the rest.
The source disk is single-sided in a double-sided drive: DISKCOPY copies two sides and reports
each track of side 1 as unreadable, as the oracle shows. A 180 KiB target is not used (writing
its side 1 fails and the copy does not end). The inputs are this repository's own bytes."""
import sys
from pathlib import Path

FILES = {
    "ZERO360.IMG": bytes(360 * 1024),
    "EXTRA.TXT": b"an extra file on the source\r\n\x1a",
}
USED = "b:mkdir:SUB,b:add:ROOT.TXT=EXTRA.TXT"
KEY = ("Strike any key when ready", " ")
NO = ("Copy another", "n")
CASES = [
    ("d01_unformatted_360", "b:image:ZERO360.IMG", "diskcopy a: b:", [KEY, NO]),
    ("d03_formatted_360", USED, "diskcopy a: b:", [KEY, NO]),
    ("d04_extra_file", "EXTRA.TXT,b:image:ZERO360.IMG", "diskcopy a: b:", [KEY, NO]),
    ("d05_another", "b:image:ZERO360.IMG", "diskcopy a: b:",
     [KEY, ("Copy another", "y"), ("$Strike any key when ready", " "), ("$Copy another (Y/N)?", "n")]),
    ("d08_one_drive", USED, "diskcopy b: b:",
     [("$Strike any key when ready", " ")] * 4 + [NO]),
    ("d06_bad_drive", "-", "diskcopy a: q:", []),
    ("d07_extra_argument", "b:image:ZERO360.IMG", "diskcopy a: b: x", [KEY, NO]),
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
