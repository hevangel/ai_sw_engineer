#!/usr/bin/env python3
"""Split a 4004 ROM image into the five 256-byte 4001 mask hex files the
ai_hw_engineer board loads via $readmemh (src/rom/rom_4001_N.hex, one hex byte
per line, chip 0 first). Images shorter than 1280 bytes are zero-padded; images
up to 4096 bytes use 4001 chips 0..15 (the architectural max), but the committed
board straps only five 4001s, so chips 5..15 are written for completeness only
when present and ignored by the 5-ROM board.
"""
import os
import sys


def main():
    if len(sys.argv) != 3:
        sys.exit("usage: split_rom.py <image.bin> <out_dir>")
    image, out_dir = sys.argv[1], sys.argv[2]
    data = open(image, "rb").read()
    if not (1280 <= len(data) <= 4096):
        sys.exit(f"image size {len(data)} out of range (1280..4096)")
    os.makedirs(out_dir, exist_ok=True)
    chips = (len(data) + 255) // 256
    chips = max(chips, 5)  # always emit at least the five strapped masks
    for chip in range(chips):
        seg = data[chip * 256:(chip + 1) * 256]
        seg = seg + bytes(256 - len(seg))
        path = os.path.join(out_dir, f"rom_4001_{chip}.hex")
        with open(path, "w", newline="\n") as fh:
            fh.write("".join("%02x\n" % b for b in seg))
    print(f"split {len(data)} bytes into {chips} x 4001 mask hex files in {out_dir}")


if __name__ == "__main__":
    main()
