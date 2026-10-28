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

    d, chains = sized()  # first cluster of F5 beyond the disk: a directory error
    struct.pack_into('<H', d, 3 * SECTOR + 5 * 32 + 26, 400)
    disks['d10_directory_error'] = d

    d, chains = sized()  # a chain entry beyond the last cluster
    fat_set(d, chains[6][2], 0x200)
    disks['d11_chain_beyond_disk'] = d

    d, chains = sized()  # a chain that loops back to its own first cluster
    fat_set(d, chains[6][-1], chains[6][0])
    disks['d12_chain_loop'] = d

    d, chains = sized()  # a file with a size but no clusters
    struct.pack_into('<I', d, 3 * SECTOR + 0 * 32 + 28, 1000)
    disks['d13_no_clusters'] = d

    d, chains = sized()  # exactly one used cluster that no file owns
    fat_set(d, 310, 0xFFF)
    disks['d14_one_lost_cluster'] = d

    d, chains = sized()  # a chain entry just beyond the last cluster (315)
    fat_set(d, chains[6][2], 315)
    disks['d15_chain_to_315'] = d

    d, chains = sized()  # a first cluster just beyond the last cluster
    struct.pack_into('<H', d, 3 * SECTOR + 5 * 32 + 26, 315)
    disks['d16_first_cluster_315'] = d

    d, chains = sized()  # a chain that ends with FF8h, the lowest end-of-chain value
    fat_set(d, chains[6][-1], 0xFF8)
    disks['d17_end_ff8'] = d

    d, chains = sized()  # a chain entry FF7h (bad cluster) inside the chain
    fat_set(d, chains[6][2], 0xFF7)
    disks['d18_chain_ff7'] = d

    d, chains = sized()  # an unowned used cluster at the very end of the FAT
    fat_set(d, 314, 0xFFF)
    disks['d19_lost_last_cluster'] = d

    for name, img in disks.items():
        assert len(img) == 160 * 1024
        with open(f'{out}/{name}.img', 'wb') as f:
            f.write(img)
    print('wrote', ', '.join(disks))


if __name__ == '__main__':
    main()
