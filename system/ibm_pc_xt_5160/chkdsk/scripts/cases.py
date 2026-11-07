#!/usr/bin/env python3
"""The CHKDSK test cases: drive B disks (exact bytes) and what is typed for each.

    python3 -I scripts/cases.py DIR    # writes the disks and files to DIR; prints one
                                       # tab-separated line per case: name, setup, command,
                                       # then pairs of (text to wait for, keys to type)

CHKDSK runs from drive A (PC DOS 2.00 disk 1) and checks the disk in drive B. A setup item
`b:image:NAME.IMG` makes DIR/NAME.IMG, built here, the drive B disk; the other `b:` steps
(mkdir, label, add) are pcdisk's, on a blank 360 KiB disk (scripts/capture_dos.sh). The damaged
disks are written byte by byte here in pcdisk's 360 KiB layout: boot sector with a DOS 2 BPB,
two 2-sector FATs (sectors 1-4), a 7-sector root directory (5-11, 112 entries), 2-sector
clusters from sector 12, media byte FDh. c08 runs CHKDSK /F, then CHKDSK /V on the fixed disk
(a `$` wait: the last non-blank row ends with the rest). The inputs are this repository's own
bytes."""
import struct
import sys
from pathlib import Path

SECTOR = 512
CLUSTER = 2 * SECTOR
BPB = bytes.fromhex("eb3c9049424d2020322e300002020100027000d002fd0200090002000000")


def blank():
    d = bytearray(b"\xf6" * (360 * 1024))
    d[:SECTOR] = bytes(SECTOR)
    d[: len(BPB)] = BPB
    d[510:512] = b"\x55\xaa"
    for fat in (1, 3):
        d[fat * SECTOR : (fat + 2) * SECTOR] = bytes(2 * SECTOR)
        d[fat * SECTOR : fat * SECTOR + 3] = b"\xfd\xff\xff"
    d[5 * SECTOR : 12 * SECTOR] = b"\xe5" * (7 * SECTOR)
    return d


def fat_set(d, n, v):
    for fat in (1, 3):
        at = fat * SECTOR + n * 3 // 2
        w = d[at] | d[at + 1] << 8
        w = (w & 0xF000) | v if n % 2 == 0 else (w & 0x000F) | v << 4
        d[at], d[at + 1] = w & 0xFF, w >> 8


def chain(d, clusters):
    for a, b in zip(clusters, clusters[1:] + [0xFFF]):
        fat_set(d, a, b)


def cluster_at(n):
    return (12 + (n - 2) * 2) * SECTOR


def entry(name, attr, first, size):
    base, _, ext = name.partition(".")
    raw = base.ljust(8).encode() + ext.ljust(3).encode()
    # time 12:00:00, date 1983-03-08 (a fixed stamp, like the DOS disk's files)
    return raw + bytes([attr]) + bytes(10) + struct.pack("<HHHI", 12 << 11, (3 << 9) | (3 << 5) | 8, first, size)


def put(d, slot, ent):
    d[5 * SECTOR + 32 * slot : 5 * SECTOR + 32 * slot + 32] = ent


def fill(d, clusters, byte):
    for c in clusters:
        d[cluster_at(c) : cluster_at(c) + CLUSTER] = bytes([byte]) * CLUSTER


def disks():
    out = {}
    # Two good files, one lost chain of 3 clusters (6..8) no entry points at.
    d = blank()
    chain(d, [2, 3]); fill(d, [2, 3], 0x41); put(d, 0, entry("GOOD.TXT", 0x20, 2, 2000))
    chain(d, [4]); fill(d, [4], 0x42); put(d, 1, entry("SMALL.DAT", 0x20, 4, 100))
    chain(d, [6, 7, 8]); fill(d, [6, 7, 8], 0x43)
    out["LOST.IMG"] = d
    # Two files whose chains meet at cluster 5 (cross-linked).
    d = blank()
    chain(d, [2, 3, 5]); put(d, 0, entry("FIRST.TXT", 0x20, 2, 3000))
    chain(d, [4, 5]); put(d, 1, entry("SECOND.TXT", 0x20, 4, 2000))
    out["CROSS.IMG"] = d
    # A file whose size needs more clusters than its chain has, and one with too many.
    d = blank()
    chain(d, [2]); put(d, 0, entry("SHORT.TXT", 0x20, 2, 5000))
    chain(d, [3, 4, 5, 6]); put(d, 1, entry("LONG.TXT", 0x20, 3, 100))
    out["SIZE.IMG"] = d
    # A chain that points past the last cluster (invalid cluster), and a free cluster in one.
    d = blank()
    fat_set(d, 2, 3); fat_set(d, 3, 0xF00); put(d, 0, entry("BADLINK.TXT", 0x20, 2, 3000))
    fat_set(d, 4, 0); put(d, 1, entry("FREE.TXT", 0x20, 4, 500))
    out["INVALID.IMG"] = d
    # A fragmented file and a contiguous one, for the file-name argument.
    d = blank()
    chain(d, [2, 3]); put(d, 0, entry("CONTIG.TXT", 0x20, 2, 2048))
    chain(d, [4, 6, 8]); put(d, 1, entry("FRAG.TXT", 0x20, 4, 3000))
    chain(d, [5]); put(d, 2, entry("GAP1.TXT", 0x20, 5, 10))
    chain(d, [7]); put(d, 3, entry("GAP2.TXT", 0x20, 7, 10))
    out["FRAG.IMG"] = d
    # Hidden and system files, a label, a zero-length file.
    d = blank()
    put(d, 0, entry("MYDISK", 0x08, 0, 0))
    chain(d, [2, 3, 4]); put(d, 1, entry("HIDDEN.SYS", 0x06, 2, 2500))
    chain(d, [5]); put(d, 2, entry("PLAIN.TXT", 0x20, 5, 1))
    put(d, 3, entry("EMPTY.TXT", 0x20, 0, 0))
    out["ATTR.IMG"] = d
    return out


FILES = {"ONE.TXT": b"one\r\n\x1a", "BIG.BIN": bytes(5000)}
TREE = "b:mkdir:SUB,b:mkdir:SUB\\INNER,b:add:ROOT.TXT=ONE.TXT,b:add:SUB\\A.TXT=ONE.TXT," \
    "b:add:SUB\\INNER\\BIG.BIN=BIG.BIN"
YN = "Convert lost chains"
CASES = [
    ("c01_blank", "b:mkdir:EMPTYDIR", "chkdsk b:", []),
    ("c02_tree", TREE, "chkdsk b:", []),
    ("c03_verbose", TREE, "chkdsk b:/v", []),
    ("c04_label", TREE + ",b:label:CHECKED", "chkdsk b:", []),
    ("c05_attrs", "b:image:ATTR.IMG", "chkdsk b:/v", []),
    ("c06_lost_no", "b:image:LOST.IMG", "chkdsk b:", [(YN, "n\r")]),
    ("c07_lost_yes", "b:image:LOST.IMG", "chkdsk b:", [(YN, "y\r")]),
    ("c08_lost_fix", "b:image:LOST.IMG", "chkdsk b:/f", [(YN, "y\r"), ("$A>", "chkdsk b:/v\r")]),
    ("c09_cross", "b:image:CROSS.IMG", "chkdsk b:", []),
    ("c10_size", "b:image:SIZE.IMG", "chkdsk b:", []),
    ("c11_invalid", "b:image:INVALID.IMG", "chkdsk b:", []),
    ("c12_contiguous", "b:image:FRAG.IMG", "chkdsk b:*.txt", []),
    ("c13_one_file", "b:image:FRAG.IMG", "chkdsk b:contig.txt", []),
    ("c14_bad_drive", "-", "chkdsk q:", []),
    # The default drive: B is made current and CHKDSK run from A without a drive argument (on
    # drive A itself the result would count CHKDSK.COM's own clusters, which a rewrite changes).
    ("c15_default", TREE, "b:", [("B>", "a:chkdsk\r"), ("$B>", "a:\r")]),
]


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    for name, data in {**FILES, **disks()}.items():
        (out / name).write_bytes(data)
    for name, setup, cmd, pairs in CASES:
        print("\t".join([name, setup, cmd] + [x for p in pairs for x in p]))


if __name__ == "__main__":
    main()
