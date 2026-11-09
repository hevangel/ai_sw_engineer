#!/usr/bin/env python3
"""The EDLIN test cases: input files (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

EDLIN is interactive: each case starts it on a file of drive A, types its commands at the `*`
prompt (and lines at the `n:*` insert prompt), ends with E (write, keeping NAME.BAK) or Q, and
then types the result with TYPE at the next A> prompt, so the screen shows what EDLIN wrote. A
wait text starting with `$` waits for the last non-blank row to end with the rest, once the
screen has changed. `^Z` and `^C` in keys are Ctrl+Z and Ctrl+C (in R, ^Z separates the two
strings). The inputs are this repository's own bytes."""
import sys
from pathlib import Path


def text(*lines):
    return b"".join(l.encode() + b"\r\n" for l in lines) + b"\x1a"


FRUIT = ("apple", "banana", "cherry", "date", "elderberry")
FILES = {
    "FRUIT.TXT": text(*FRUIT),
    "OTHER.TXT": text("one", "two"),
    "LONG.TXT": text(*["line %02d" % i for i in range(1, 41)]),
    "KEEP.BAK": text("a backup"),
}
P = "$*"
Q = "$?"  # O.K.? and Abort edit (Y/N)? end with a question mark


def run(*cmds):
    return [(P, c) for c in cmds]


def then_type(*names):
    return [("$A>", "type %s\r" % n) for n in names]


CASES = [
    ("e01_new_file", "-", "edlin new.txt",
     run("i\r", "first line\r", "second line\r", "^Z\r", "l\r", "e\r") + then_type("new.txt")),
    ("e02_delete", "FRUIT.TXT", "edlin fruit.txt",
     run("2d\r", "l\r", "e\r") + then_type("fruit.txt", "fruit.bak")),
    ("e03_search", "FRUIT.TXT", "edlin fruit.txt",
     run("1,5san\r", "3,5?serr\r") + [(Q, "y")] + run("q\r") + [(Q, "y")]),
    ("e04_replace", "FRUIT.TXT", "edlin fruit.txt",
     run("1,5rerr^Zxx\r", "l\r", "e\r") + then_type("fruit.txt")),
    ("e05_insert_middle", "FRUIT.TXT", "edlin fruit.txt",
     run("3i\r", "inserted\r", "^C", "l\r", "e\r") + then_type("fruit.txt")),
    ("e06_move_copy", "FRUIT.TXT", "edlin fruit.txt",
     run("1,2,5m\r", "l\r", "1,1,1,2c\r", "l\r", "e\r") + then_type("fruit.txt")),
    ("e07_quit", "FRUIT.TXT", "edlin fruit.txt",
     run("1,3d\r", "q\r") + [(Q, "y")] + then_type("fruit.txt")),
    ("e08_page", "LONG.TXT", "edlin long.txt", run("1p\r", "p\r", "q\r") + [(Q, "y")]),
    ("e09_transfer", "FRUIT.TXT,OTHER.TXT", "edlin fruit.txt",
     run("3tother.txt\r", "l\r", "e\r") + then_type("fruit.txt")),
    ("e10_edit_line", "FRUIT.TXT", "edlin fruit.txt",
     run("2\r", "BANANA\r", ".,#l\r", "e\r") + then_type("fruit.txt")),
    ("e11_ranges", "LONG.TXT", "edlin long.txt", run("20\r", "\r", "-2,+2l\r", "#l\r", "35,l\r", "q\r") + [(Q, "y")]),
    ("e12_entry_error", "FRUIT.TXT", "edlin fruit.txt", run("zz\r", "9d\r", "l\r", "q\r") + [(Q, "y")]),
    ("e13_no_name", "-", "edlin", []),
    ("e14_bak_file", "KEEP.BAK", "edlin keep.bak", []),
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
