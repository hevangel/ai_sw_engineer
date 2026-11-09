#!/usr/bin/env python3
"""The DEBUG test cases: input files (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the files to DIR; prints one tab-separated line
                                       # per case: name, files, command, then pairs of
                                       # (text to wait for, keys to type)

DEBUG.COM is on PC DOS 2.00 disk 2; the item `a:disk2:DEBUG.COM` copies it onto drive A
(scripts/capture_dos.sh). Each case starts DEBUG, types its commands at the `-` prompt (and at
the `:` and `-` prompts of R and A and the address prompts of E), and ends with Q; a case that
writes a file then TYPEs it. No case dumps or disassembles the ROMs, so no ROM bytes appear on
the screen. A wait text starting with `$` waits for the last non-blank row to end with the rest,
once the screen has changed. The inputs are this repository's own bytes."""
import sys
from pathlib import Path

FILES = {"FRUIT.TXT": b"apple\r\nbanana\r\ncherry\r\n\x1a"}
D = "a:disk2:DEBUG.COM"
P = "$-"


def run(*cmds):
    return [(P, c) for c in cmds]


def asm(addr, *lines):
    # A prompts with the address (ssss:oooo and a space, which the screen rows do not keep); a
    # bare `$` waits for the screen to change. An empty line ends A.
    return [(P, "a %s\r" % addr)] + [("$", l + "\r") for l in lines] + [("$", "\r")]


CASES = [
    ("g01_registers", D, "debug", run("r\r", "q\r")),
    ("g02_enter_dump", D, "debug", run("e 200 41 42 43 \"xyz\" 0d 0a\r", "d 200 l 20\r", "q\r")),
    ("g03_fill_search", D, "debug", run("f 300 l 20 55 aa\r", "s 300 l 40 55 aa\r", "s 300 l 40 99\r", "q\r")),
    ("g04_move_compare", D, "debug",
     run("f 400 l 10 1\r", "m 400 l 10 500\r", "e 505 9\r", "c 400 l 10 500\r", "q\r")),
    ("g05_hex", D, "debug", run("h 1234 abcd\r", "h ffff 1\r", "q\r")),
    ("g06_assemble", D, "debug",
     asm("100", "mov ax,1234", "add ax,bx", "mov [bx+si+10],al", "jmp 100", "int 20")
     + run("u 100 l 10\r", "q\r")),
    ("g07_trace_go", D, "debug",
     asm("100", "mov ax,5", "inc ax", "mov cx,ax", "int 20") + run("t\r", "t\r", "g\r", "q\r")),
    ("g08_register_edit", D, "debug",
     run("r ax\r") + [("$:", "1234\r")] + run("r f\r") + [("$-", "ov dn\r")] + run("r\r", "q\r")),
    ("g09_load_file", D + ",FRUIT.TXT", "debug fruit.txt", run("d 100 l 20\r", "r\r", "q\r")),
    ("g10_write_file", D, "debug",
     run("n out.dat\r", "e 100 \"hello\" 0d 0a\r", "r cx\r") + [("$:", "7\r")] + run("w\r", "q\r")
     + [("$A>", "type out.dat\r")]),
    ("g11_errors", D, "debug", run("zz\r", "d 100 l\r", "e\r", "q\r")),
    ("g12_load_exe", D, "debug sort.exe", run("r\r", "u l 6\r", "q\r")),
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
