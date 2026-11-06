#!/usr/bin/env python3
"""The FIND test cases: input files (exact bytes) and the command typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints name<TAB>files<TAB>command

The inputs are this repository's own text."""
import sys
from pathlib import Path

CRLF, EOF = b"\r\n", b"\x1a"


def lines(*ls, end=CRLF + EOF):
    return CRLF.join(l.encode("latin-1") for l in ls) + end


FILES = {
    "FRUIT.TXT": lines("pear", "Apple", "banana", "apple", "Banana", "cherry", "", "plantain", " mango"),
    "A.TXT": lines("alpha", "beta", "gamma"),
    "B.TXT": lines("delta", "epsilon", "zeta", "eta"),
    "QUOTE.TXT": lines('say "hi"', "no quotes", '""', 'end"'),
    "NONL.TXT": lines("one an", "two", "three an", end=b""),
    "LONG.TXT": lines("x" * 200 + "an", "short an", "y" * 300),
    "CTRLZ.TXT": lines("before an", end=CRLF + EOF + b"after an" + CRLF),
}

CASES = [
    ("f01_basic", "FRUIT.TXT", 'find "an" fruit.txt'),
    ("f02_invert", "FRUIT.TXT", 'find /v "an" fruit.txt'),
    ("f03_count", "FRUIT.TXT", 'find /c "an" fruit.txt'),
    ("f04_number", "FRUIT.TXT", 'find /n "an" fruit.txt'),
    ("f05_count_invert", "FRUIT.TXT", 'find /c /v "an" fruit.txt'),
    ("f06_number_invert", "FRUIT.TXT", 'find /N /V "an" fruit.txt'),
    ("f07_case", "FRUIT.TXT", 'find "Apple" fruit.txt'),
    ("f08_two_files", "A.TXT,B.TXT", 'find "ta" a.txt b.txt'),
    ("f09_not_found", "A.TXT", 'find "ta" a.txt nofile.txt'),
    ("f10_no_string", "A.TXT", "find a.txt"),
    ("f11_unquoted", "A.TXT", "find ta a.txt"),
    ("f12_quote_in_string", "QUOTE.TXT", 'find """" quote.txt'),
    ("f13_empty_string", "A.TXT", 'find "" a.txt'),
    ("f14_stdin", "FRUIT.TXT", 'find "an" <fruit.txt'),
    ("f15_pipe", "FRUIT.TXT", 'type fruit.txt | find /n "e"'),
    ("f16_bad_option", "A.TXT", 'find /x "ta" a.txt'),
    ("f17_no_newline", "NONL.TXT", 'find /n "an" nonl.txt'),
    ("f18_long_lines", "LONG.TXT", 'find /c "an" long.txt'),
    ("f19_ctrl_z", "CTRLZ.TXT", 'find "an" ctrlz.txt'),
    ("f20_no_args", "-", "find"),
    ("f21_stdin_count", "FRUIT.TXT", 'find /c "e" <fruit.txt'),
    ("f22_space_string", "FRUIT.TXT", 'find " " fruit.txt'),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, data in FILES.items():
        (out / name).write_bytes(data)
    for name, files, cmd in CASES:
        print(f"{name}\t{files}\t{cmd}")


if __name__ == "__main__":
    main()
