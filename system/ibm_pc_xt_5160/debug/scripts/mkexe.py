#!/usr/bin/env python3
"""mkexe.py <image.bin> <map> <out>

Wraps the flat image i8086-ld wrote (linked at 100h by link_com.ld) into the load-high MZ file
the rewrite is shipped as, under the name DEBUG.COM (spec D1). PC DOS 2.00 runs a file whose
first two bytes are `MZ` as an .EXE whatever its name, and loads an .EXE whose MINALLOC and
MAXALLOC are both 0 at the top of memory, leaving the program segment prefix's 64 KiB, where
the core keeps the original's data and text, as it was. Memory past the image is then not the
program's, so the file holds .bss and the stack as zeros: the image is padded to the end of
the stack (`__bss_end` from the map, plus STACK as in src/start.s). The header is 32 bytes
(two paragraphs, no relocations): CS = SS = FFF0h, so that DOS starts the image at the offset
100h it was linked at, IP = 100h (`_start`, the first byte), SP = the end of the stack."""
import re
import struct
import sys

STACK = 0x800  # src/start.s
HEADER = 32

image = open(sys.argv[1], "rb").read()
bss_end = int(re.search(r"0x([0-9a-f]+) __bss_end", open(sys.argv[2]).read()).group(1), 16)
top = (bss_end + STACK) & ~1
assert top <= 0xFFFE, "the image, .bss and stack must fit in the segment"
image += bytes(top - 0x100 - len(image))
total = HEADER + len(image)
h = bytearray(HEADER)
h[0:2] = b"MZ"
struct.pack_into(
    "<HHHHHHHHHHHH", h, 2,
    total % 512,           # bytes in the last page
    (total + 511) // 512,  # pages
    0,                     # relocations
    HEADER // 16,          # header paragraphs
    0, 0,                  # MINALLOC, MAXALLOC: load high
    0xFFF0, top,           # SS:SP
    0,                     # checksum
    0x100, 0xFFF0,         # CS:IP
    0x1C,                  # relocation table
)
open(sys.argv[3], "wb").write(bytes(h) + image)
