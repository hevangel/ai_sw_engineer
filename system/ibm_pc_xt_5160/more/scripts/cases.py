#!/usr/bin/env python3
"""The MORE test cases: input files (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

MORE pages standard input: after a screenful it prints "-- More --" and waits for a key. A wait
text starting with `$` waits for the last non-blank row to end with the rest, so each new prompt
at the bottom of the screen is waited for. The inputs are this repository's own bytes."""
import sys
from pathlib import Path


def lines(n, fmt="Line {:02}"):
    return b"".join(fmt.format(i).encode() + b"\r\n" for i in range(1, n + 1))


FILES = {
    "SHORT.TXT": lines(5) + b"\x1a",
    "LONG.TXT": lines(60) + b"\x1a",
    "WIDE.TXT": b"".join((b"%02d" % i) + b"-" * 98 + b"\r\n" for i in range(1, 31)) + b"\x1a",
    "TABS.TXT": b"a\tb\tc\r\n\tindented\r\n12345678\tnine\r\nx\x08y\x07z\r\n\x1a",
    "CTRLZ.TXT": b"before\r\n\x1aafter\r\n",
    "L23.TXT": lines(23),
    "L24.TXT": lines(24),
    "L25.TXT": lines(25),
    "L80.TXT": b"".join((b"%02d" % i) + b"=" * 78 + b"\r\n" for i in range(1, 31)),
}
P = "$-- More --"
CASES = [
    ("m01_short", "SHORT.TXT", "more < short.txt", []),
    ("m02_long", "LONG.TXT", "more < long.txt", [(P, " "), (P, " ")]),
    ("m03_pipe", "LONG.TXT", "type long.txt | more", [(P, "x"), (P, "\r")]),
    ("m04_wide", "WIDE.TXT", "more < wide.txt", [(P, " "), (P, " ")]),
    ("m05_controls", "TABS.TXT", "more < tabs.txt", []),
    ("m06_ctrl_z", "CTRLZ.TXT", "more < ctrlz.txt", []),
    ("m07_23_lines", "L23.TXT", "more < l23.txt", []),
    ("m08_24_lines", "L24.TXT", "more < l24.txt", [(P, " ")]),
    ("m09_25_lines", "L25.TXT", "more < l25.txt", [(P, " ")]),
    ("m10_80_columns", "L80.TXT", "more < l80.txt", [(P, " "), (P, " ")]),
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
