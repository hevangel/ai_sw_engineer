#!/usr/bin/env python3
"""The SORT test cases: input files (exact bytes) and the command typed for each.

    python3 -I scripts/cases.py DIR      # writes DIR/<file> for every case, prints the table

Imported by the other scripts. The inputs are this repository's own text (no original data).
Each case is one DOS command line, typed at the A> prompt of PC DOS 2.00 with the input file on
drive A; the oracle is the screen rows the command prints (oracle/transcripts/<case>.txt).
"""
import sys
from pathlib import Path

CRLF, EOF = b"\r\n", b"\x1a"


def lines(*ls, end=CRLF + EOF):
    """Lines joined with CR LF, then `end` (default: a final CR LF and ^Z, as EDLIN writes)."""
    return CRLF.join(l.encode("latin-1") for l in ls) + end


FRUIT = lines("pear", "Apple", "banana", "apple", "Banana", "cherry", "", "pear", " zebra", "123")
COLS = lines("b 3 x", "a 1 z", "c 2 y", "a 3 w", "b 1 v")
TIES = lines("1 same", "2 same", "3 same", "4 other", "5 same")
SHORT = lines("abcdefg", "ab", "abcdzzz", "", "abcdaaa", "abc")
HIGH = lines("\x82clair", "eclair", "Eclair", "~tilde", "\x7fdel", "zz")

# (name, file, contents, command)
CASES = [
    ("c01_basic", "FRUIT.TXT", FRUIT, "sort <fruit.txt"),
    ("c02_reverse", "FRUIT.TXT", FRUIT, "sort /r <fruit.txt"),
    ("c03_column", "COLS.TXT", COLS, "sort /+3 <cols.txt"),
    ("c04_plus_noslash", "COLS.TXT", COLS, "sort +5 <cols.txt"),
    ("c05_column_rev", "COLS.TXT", COLS, "sort /R/+3 <cols.txt"),
    ("c06_plus0", "COLS.TXT", COLS, "sort /+0 <cols.txt"),
    ("c07_ties", "TIES.TXT", TIES, "sort /+3 <ties.txt"),
    ("c08_ties_rev", "TIES.TXT", TIES, "sort /r /+3 <ties.txt"),
    ("c09_short", "SHORT.TXT", SHORT, "sort /+5 <short.txt"),
    ("c10_no_eof", "NOEOF.TXT", lines("delta", "alpha", "charlie", "bravo", end=CRLF), "sort <noeof.txt"),
    ("c11_no_newline", "NONL.TXT", lines("delta", "alpha", "charlie", "bravo", end=b""), "sort <nonl.txt"),
    ("c12_ctrlz_only", "ZEND.TXT", lines("delta", "alpha", "bravo", end=EOF), "sort <zend.txt"),
    ("c13_empty", "EMPTY.TXT", b"", "sort <empty.txt"),
    ("c14_high", "HIGH.TXT", HIGH, "sort <high.txt"),
    ("c15_pipe", "FRUIT.TXT", FRUIT, "type fruit.txt | sort /r"),
    ("c16_bigcol", "COLS.TXT", COLS, "sort /+70000 <cols.txt"),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, f, data, cmd in CASES:
        (out / f).write_bytes(data)
        print(f"{name:18} {f:10} {len(data):4} bytes  {cmd}")


if __name__ == "__main__":
    main()
