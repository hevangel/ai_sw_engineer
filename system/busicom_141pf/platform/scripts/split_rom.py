#!/usr/bin/env python3
"""Split a 4004 ROM image into the 256-byte 4001 mask hex files the
ai_hw_engineer board loads via $readmemh (src/rom/rom_4001_N.hex, one hex byte
per line, chip 0 first). Images shorter than the chip count are zero-padded.

By default at least five masks are emitted (the stock board straps five 4001s).
Pass --chips N to force N masks (e.g. 16 for the xl board, whose RTL top
instantiates sixteen 4001s and $readmemh-loads every mask slot); high chips with
no image bytes are written as all-zero masks.
"""
import os
import sys


def main():
    args = [a for a in sys.argv[1:]]
    want_chips = None
    if "--chips" in args:
        i = args.index("--chips")
        want_chips = int(args[i + 1])
        del args[i:i + 2]
    if len(args) != 2:
        sys.exit("usage: split_rom.py <image.bin> <out_dir> [--chips N]")
    image, out_dir = args[0], args[1]
    data = open(image, "rb").read()
    if not (1280 <= len(data) <= 4096):
        sys.exit(f"image size {len(data)} out of range (1280..4096)")
    os.makedirs(out_dir, exist_ok=True)
    chips = (len(data) + 255) // 256
    chips = max(chips, 5)  # always emit at least the five strapped masks
    if want_chips is not None:
        chips = max(chips, want_chips)
    for chip in range(chips):
        seg = data[chip * 256:(chip + 1) * 256]
        seg = seg + bytes(256 - len(seg))
        path = os.path.join(out_dir, f"rom_4001_{chip}.hex")
        with open(path, "w", newline="\n") as fh:
            fh.write("".join("%02x\n" % b for b in seg))
    print(f"split {len(data)} bytes into {chips} x 4001 mask hex files in {out_dir}")


if __name__ == "__main__":
    main()
