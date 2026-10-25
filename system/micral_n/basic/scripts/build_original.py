#!/usr/bin/env python3
"""Build the oracle: the original SCELBAL with the Micral N teletype.

    build_original.py [out.bin]   (default build/original.bin)

The image is the bytes of the AS8 listing original/sc1.lst (original/fetch.sh),
which are the 1974-75 program at its documented addresses (the listing loads
at 000 100 and starts there), with oracle/micral_io.asm overlaid: a jump at 0
and the page-0 serial routines CINP and CPRINT. Only the bytes that file
assembles are replaced. Assembled by platform/scripts/as8008.py.
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TITLE = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(TITLE, "..", "platform", "scripts"))
from as8008 import Asm  # noqa: E402


def listing_bytes(path):
    """{address: byte} from an AS8 listing: columns 5-10 hold PP-OOO (octal
    page and offset), columns 12-22 up to three octal code bytes."""
    mem = {}
    for line in open(path, encoding="latin-1"):
        m = re.fullmatch(r"([0-7]{2})-([0-7]{3})", line[5:11])
        if not m:
            continue
        addr = int(m.group(1), 8) << 8 | int(m.group(2), 8)
        for i, t in enumerate(line[12:23].split()):
            mem[addr + i] = int(t, 8)
    return mem


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(TITLE, "build", "original.bin")
    lst = os.path.join(TITLE, "original", "sc1.lst")
    if not os.path.exists(lst):
        subprocess.run([os.path.join(TITLE, "original", "fetch.sh")], check=True)
    mem = listing_bytes(lst)
    lines = open(os.path.join(TITLE, "oracle", "micral_io.asm")).read().splitlines()
    asm = Asm()
    asm.run(lines, need=False)
    patch, _ = asm.run(lines, need=True)
    mem.update(patch)
    img = bytes(mem.get(i, 0) for i in range(max(mem) + 1))
    os.makedirs(os.path.dirname(out), exist_ok=True)
    open(out, "wb").write(img)
    print(f"build_original: {out}: {len(img)} B, {len(patch)} B patched")


if __name__ == "__main__":
    main()
