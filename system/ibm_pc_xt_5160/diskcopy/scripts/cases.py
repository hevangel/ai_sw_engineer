#!/usr/bin/env python3
"""The DISKCOPY test cases: drive A and B disks and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the disks and files to DIR; prints one
                                       # tab-separated line per case: name, setup, command,
                                       # then pairs of (text to wait for, keys to type)

DISKCOPY runs from drive A and copies a disk to the disk in drive B. For the A-to-B cases drive A
is `a:system360`: a double-sided 360 KiB copy of PC DOS 2.00 disk 1 that capture_dos.sh builds
with pcdisk syscopy (disk 1 itself is single-sided, and reading side 1 of a one-sided image is
where MartyPC and the hardware differ: MartyPC ends the read with ST0=40h and no ST1 bits, the
BIOS returns 20h, and DISKCOPY 2.0 then copies two sides; a real one-sided disk gives "address
mark not found", 02h, which DISKCOPY takes for one side). `b:image:NAME` makes DIR/NAME, written
here, the drive B disk; other `b:` steps are pcdisk's on a blank 360 KiB disk. Each capture also
records the SHA-256 of drive B after the run (SAVE_B=1); d05 copies twice and needs
MARTY_CYCLES=1500000000 on MartyPC. A wait text starting with `$` waits for the last non-blank
row to end with the rest. The inputs are this repository's own bytes."""
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
    ("d01_unformatted_360", "a:system360,b:image:ZERO360.IMG", "diskcopy a: b:", [KEY, NO]),
    ("d03_formatted_360", "a:system360," + USED, "diskcopy a: b:", [KEY, NO]),
    ("d04_extra_file", "a:system360,EXTRA.TXT,b:image:ZERO360.IMG", "diskcopy a: b:", [KEY, NO]),
    ("d05_another", "a:system360,b:image:ZERO360.IMG", "diskcopy a: b:",
     [KEY, ("Copy another", "y"), ("$Strike any key when ready", " "), ("$Copy another (Y/N)?", "n")]),
    ("d08_one_drive", USED, "diskcopy b: b:",
     [("$Strike any key when ready", " ")] * 4 + [NO]),
    ("d06_bad_drive", "-", "diskcopy a: q:", []),
    ("d07_extra_argument", "a:system360,b:image:ZERO360.IMG", "diskcopy a: b: x", [KEY, NO]),
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
