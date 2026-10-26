#!/usr/bin/env python3
"""Build the test disks D3..D9 of plans/testplan.md into <out_dir> (160 KiB PC DOS 1.x images).

Layout (see platform/tools/pcdisk): FATs at sectors 1 and 2, directory at 3..6,
cluster n at sector 7 + n - 2. The damage is applied to both FAT copies.

    make_disks.py <out_dir>
"""
import struct
import sys

SECTOR = 512
SIZES = [0, 1, 511, 512, 513, 1024, 100000]


def blank():
    d = bytearray(b'\xf6' * (160 * 1024))
    d[:SECTOR] = bytes(SECTOR)
    for fat in (1, 2):
        d[fat * SECTOR:(fat + 1) * SECTOR] = bytes(SECTOR)
        d[fat * SECTOR:fat * SECTOR + 3] = b'\xfe\xff\xff'
    d[3 * SECTOR:7 * SECTOR] = b'\xe5' * (4 * SECTOR)
    return d


def fat_get(d, n):
    at = SECTOR + n * 3 // 2
    w = d[at] | d[at + 1] << 8
    return w & 0xFFF if n % 2 == 0 else w >> 4


def fat_set(d, n, v):
    for fat in (1, 2):
        at = fat * SECTOR + n * 3 // 2
        w = d[at] | d[at + 1] << 8
        w = (w & 0xF000) | v if n % 2 == 0 else (w & 0x000F) | v << 4
        d[at], d[at + 1] = w & 0xFF, w >> 8


def add(d, slot, name, size, data=None):
    need = -(-size // SECTOR)
    free = [c for c in range(2, 315) if fat_get(d, c) == 0][:need]
    for i, c in enumerate(free):
        fat_set(d, c, free[i + 1] if i + 1 < len(free) else 0xFFF)
        d[(7 + c - 2) * SECTOR:(8 + c - 2) * SECTOR] = bytes([0x30 + slot]) * SECTOR
    base, ext = name.split('.')
    e = 3 * SECTOR + slot * 32
    d[e:e + 32] = bytes(32)
    d[e:e + 11] = base.ljust(8).encode() + ext.ljust(3).encode()
    struct.pack_into('<HI', d, e + 26, free[0] if free else 0, size)
    return free


def sized():
    d = blank()
    chains = [add(d, i, f'F{i}.DAT', s) for i, s in enumerate(SIZES)]
    return d, chains


def main():
    out = sys.argv[1]
    disks = {}
    disks['d3_sizes'], chains = sized()

    d, chains = sized()  # directory size larger than the chain
    e = 3 * SECTOR + 5 * 32
    struct.pack_into('<I', d, e + 28, 5000)
    disks['d4_size_error'] = d

    d, chains = sized()  # chain cut by a free cluster in the middle of the 100000-byte file
    fat_set(d, chains[6][3], 0)
    disks['d5_allocation_error'] = d

    d, chains = sized()  # file F5 (1024 bytes) is made to share the second cluster of F6
    e = 3 * SECTOR + 5 * 32
    struct.pack_into('<H', d, e + 26, chains[6][1])
    disks['d6_cross_linked'] = d

    d, chains = sized()  # a chain of 4 used clusters that no directory entry owns
    for c, nxt in zip(range(300, 304), [301, 302, 303, 0xFFF]):
        fat_set(d, c, nxt)
    disks['d7_lost_chain'] = d

    d = blank()  # full: one file taking every cluster
    add(d, 0, 'BIG.DAT', 313 * SECTOR)
    disks['d8_full'] = d

    d, chains = sized()
    for fat in (1, 2):
        d[fat * SECTOR] = 0xFD
    disks['d9_media_byte'] = d

    for name, img in disks.items():
        assert len(img) == 160 * 1024
        with open(f'{out}/{name}.img', 'wb') as f:
            f.write(img)
    print('wrote', ', '.join(disks))


if __name__ == '__main__':
    main()
