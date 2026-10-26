#!/usr/bin/env python3
"""Make a Micral N boot disk image (io_map.md "Floppy disk").

    mkdisk.py <program.bin> <disk.img>

The raw image is track-major: 64 tracks x 32 sectors x 128 bytes. Sectors
0-15 of track 0 hold filler (0xAA, as in [MO5emu] create_virtual_disk); the
program, linked to run from address 0, starts at track 0 sector 16, where the
boot ROM begins reading (boot spec BOOT-1). The boot loads 0x3700 bytes, so
the program must be shorter.
"""
import pathlib
import sys

TRACKS, SECTORS, SIZE = 64, 32, 128
FIRST = 16
LOAD = 0x3700


def safe_out(path):
    """CLI output path: relative paths must stay inside the working
    directory (no `..` climbs); absolute paths are explicit intent."""
    p = pathlib.Path(path).resolve()
    if not pathlib.Path(path).is_absolute() and pathlib.Path.cwd() not in p.parents:
        raise SystemExit(f"mkdisk: refusing relative output outside the "
                         f"working directory: {path}")
    return p


prog = open(sys.argv[1], "rb").read()
if len(prog) > LOAD:
    sys.exit(f"mkdisk: program is {len(prog)} bytes, the boot loads {LOAD}")
img = bytearray([0xAA] * (FIRST * SIZE)) + prog
img += bytes(TRACKS * SECTORS * SIZE - len(img))
safe_out(sys.argv[2]).write_bytes(img)
print(f"mkdisk: {sys.argv[2]}: {len(prog)} B program from track 0 sector {FIRST}")
