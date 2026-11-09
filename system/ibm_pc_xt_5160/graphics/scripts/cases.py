#!/usr/bin/env python3
"""The GRAPHICS test cases: the capture tools on drive A and what is typed for each.

    python3 -I scripts/cases.py DIR    # copies the tools to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

GRAPHICS stays resident and hooks INT 5 (Print Screen). No emulator here hands over the bytes a
graphics print sends, so each case captures them inside the guest with this repository's own
tools (tools/README.md), which this script copies from build/tools (tools/build.sh builds them;
without them it says so on stderr and lists the cases all the same):
PRNCAP.COM stays resident and keeps what INT 17h is asked to print on printer 0, DRAW.COM <m>
draws the test picture in video mode m, calls INT 5 and goes back to mode 3 (showing the status
byte), and PRNDUMP.COM b:out.prn writes the kept bytes to drive B, which is a blank 360 KiB disk
(`b:label:PRINTOUT`). The capture records drive B after the run (SAVE_B=1); scripts/capture.sh
adds OUT.PRN's length and SHA-256 to each transcript. A wait text starting with `$` waits for
the last non-blank row to end with the rest, once the screen has changed. The inputs are this
repository's own bytes; GRAPHICS.COM is the one on PC DOS 2.00 disk 1 (or the rewrite)."""
import sys
from pathlib import Path

TOOLS = ["PRNCAP.COM", "PRNDUMP.COM", "DRAW.COM"]
SETUP = ",".join(TOOLS) + ",b:label:PRINTOUT"
DUMP = "prndump b:out.prn"


def then(*cmds):
    return [("$A>", c + "\r") for c in cmds]


CASES = [
    ("g01_mode6", "prncap", ["graphics", "draw 6", DUMP]),
    ("g02_mode4", "prncap", ["graphics", "draw 4", DUMP]),
    ("g03_mode5", "prncap", ["graphics", "draw 5", DUMP]),
    ("g04_text80", "prncap", ["graphics", "draw 3", DUMP]),
    ("g05_text40", "prncap", ["graphics", "draw 1", DUMP]),
    ("g06_mode6_bios", "prncap", ["draw 6", DUMP]),
    ("g07_mode4_bios", "prncap", ["draw 4", DUMP]),
    ("g08_twice", "prncap", ["graphics", "graphics", "draw 6", DUMP]),
    ("g09_busy", "prncap", ["graphics", "draw 6b", DUMP]),
    ("g10_printer_error", "prncap e", ["graphics", "draw 6", DUMP]),
    ("g11_text80_bios", "prncap", ["draw 3", DUMP]),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    built = Path(__file__).resolve().parent.parent / "build" / "tools"
    for name in TOOLS:
        src = built / name
        if src.exists():
            (out / name).write_bytes(src.read_bytes())
        else:
            print(f"cases.py: {src} is missing; run tools/build.sh first", file=sys.stderr)
    for name, cmd, more in CASES:
        print("\t".join([name, SETUP, cmd] + [x for p in then(*more) for x in p]))


if __name__ == "__main__":
    main()
