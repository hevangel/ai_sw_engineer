#!/usr/bin/env python3
"""The FORMAT test cases: drive B disks and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the disks and files to DIR; prints one
                                       # tab-separated line per case: name, setup, command,
                                       # then pairs of (text to wait for, keys to type)

FORMAT runs from drive A (PC DOS 2.00 disk 1) and formats the disk in drive B. `b:image:NAME`
makes DIR/NAME, written here, the drive B disk; other `b:` steps are pcdisk's on a blank
360 KiB disk (scripts/capture_dos.sh). Each capture also records the SHA-256 of drive B after the
run (SAVE_B=1), since what FORMAT writes is the result. A wait text starting with `$` waits for
the last non-blank row to end with the rest. The inputs are this repository's own bytes."""
import sys
from pathlib import Path

FILES = {
    "ZERO360.IMG": bytes(360 * 1024),
    "ZERO180.IMG": bytes(180 * 1024),
    "ONE.TXT": b"one\r\n\x1a",
}
USED = "b:mkdir:SUB,b:add:ROOT.TXT=ONE.TXT,b:add:SUB\\A.TXT=ONE.TXT"
KEY = ("strike any key", " ")
NO = ("Format another", "n")
CASES = [
    ("f01_plain", "b:image:ZERO360.IMG", "format b:", [KEY, NO]),
    ("f02_used_disk", USED, "format b:", [KEY, NO]),
    ("f03_system", "b:image:ZERO360.IMG", "format b:/s", [KEY, NO]),
    ("f04_volume", "b:image:ZERO360.IMG", "format b:/v", [KEY, ("Volume label", "MYDISK\r"), NO]),
    ("f05_single_side", "b:image:ZERO360.IMG", "format b:/1", [KEY, NO]),
    ("f06_eight_sectors", "b:image:ZERO360.IMG", "format b:/8", [KEY, NO]),
    ("f07_160k", "b:image:ZERO180.IMG", "format b:/1/8", [KEY, NO]),
    ("f08_system_volume", "b:image:ZERO360.IMG", "format b:/s/v", [KEY, ("Volume label", "SYS DISK\r"), NO]),
    ("f09_bad_label", "b:image:ZERO360.IMG", "format b:/v",
     [KEY, ("Volume label", "BAD.NAME\r"), ("$Volume label (11 characters, ENTER for none)?", "GOOD\r"), NO]),
    ("f10_another", "b:image:ZERO360.IMG", "format b:", [KEY, ("Format another", "y"), ("$strike any key when ready", " "), ("$Format another (Y/N)?", "n")]),
    ("f11_bad_drive", "-", "format q:", []),
    ("f12_bad_param", "b:image:ZERO360.IMG", "format b:/x", []),
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
