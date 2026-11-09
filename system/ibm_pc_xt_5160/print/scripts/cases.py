#!/usr/bin/env python3
"""The PRINT test cases: input files (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

PRINT is a background print spooler: the first run installs its resident part (asking for the
list device; PRINT 2.00 has no /D: switch, p03 shows what it does with one) and queues the files; they print while COMMAND waits at A>.
The oracle is 86Box's XT with a text printer on LPT1 (ORACLE=86box, scripts/xt_86box.py): the
transcript holds the screen and the printed lines (`#lpt|`). Every case lets the printing run to
the end (the capture settles), and none looks at the queue while a file is printing, since how
far the background printing has got at a given moment depends on each emulator's timing. A wait
text starting with `$` waits for the last non-blank row to end with the rest, once the screen has
changed. The inputs are this repository's own bytes."""
import sys
from pathlib import Path


def text(*lines):
    return b"".join(l.encode() + b"\r\n" for l in lines) + b"\x1a"


FILES = {
    "FRUIT.TXT": text("apple", "banana", "cherry"),
    "TABS.TXT": text("a\tb\tc", "\tindented", "no newline at the end") [:-3] + b"\x1a",
    "LONG.TXT": text(*["line %02d of a longer file" % i for i in range(1, 71)]),
    "FF.TXT": text("page one") [:-1] + b"\x0cpage two\r\n\x1a",
}
DEV = ("Name of list device", "\r")
CASES = [
    ("p01_one_file", "FRUIT.TXT", "print fruit.txt", [DEV]),
    ("p02_two_files", "FRUIT.TXT,TABS.TXT", "print fruit.txt tabs.txt", [DEV]),
    ("p03_d_switch", "FRUIT.TXT", "print /d:prn fruit.txt", [DEV]),
    ("p04_long_file", "LONG.TXT", "print long.txt", [DEV]),
    ("p05_form_feed", "FF.TXT", "print ff.txt", [DEV]),
    ("p06_wildcard", "FRUIT.TXT,TABS.TXT", "print *.txt", [DEV]),
    ("p07_not_found", "FRUIT.TXT", "print nofile.txt", [DEV]),
    ("p08_install_only", "-", "print", [DEV]),
    ("p09_bad_switch", "FRUIT.TXT", "print /x fruit.txt", [DEV]),
    ("p10_device_typed", "FRUIT.TXT", "print fruit.txt", [("Name of list device", "lpt1\r")]),
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
