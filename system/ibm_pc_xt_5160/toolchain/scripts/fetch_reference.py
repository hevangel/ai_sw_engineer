#!/usr/bin/env python3
"""Fetch the original IBM PC XT 5160 firmware and the PC DOS 2.00 system diskette from the
pinned PCjs revisions into a local directory, convert the disk to flat bytes and check every
SHA-256 (spec/profile.md, "Original software"). Nothing fetched here may be committed.

    python3 -I system/ibm_pc_xt_5160/toolchain/scripts/fetch_reference.py DIR
    export XT_ROMS=DIR

Standard library only.
"""

import hashlib
import json
import struct
import sys
import urllib.request
from pathlib import Path

PCJS = ("https://raw.githubusercontent.com/jeffpar/pcjs/"
        "1a91d38eb12e5fdf91116b21ad9ef1935710a496/machines/pcx86/ibm/5160/rom")
DISKS = ("https://raw.githubusercontent.com/jeffpar/pcjs-diskettes/"
         "cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00")

FILES = {
    # IBM 5160 BIOS of 1982-11-08 (8 KiB, FE000h) and Cassette BASIC C1.10 (32 KiB, F6000h).
    "xtbios.bin": (PCJS + "/bios/1982-11-08/XTBIOS-REV1.rom",
                   "2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2"),
    "xtbasic.bin": (PCJS + "/basic/BASIC110.rom",
                    "3033d1a54c99d7e2aa1fc7c8c2e51a56ae1b61bf4e70e8aa580f43c43e37a63e"),
    # PC DOS 2.00 disk 1, 180 KiB single-sided (9 sectors a track).
    "pcdos200.img": (DISKS + "/PCDOS200-DISK1.json",
                     "934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88"),
    # PC DOS 2.00 disk 2 (DEBUG, LINK, EXE2BIN, BASIC samples), 180 KiB single-sided.
    "pcdos200d2.img": (DISKS + "/PCDOS200-DISK2.json",
                       "2b80a8a3554d29706444aa1a9928a32aca70b10b4d88e733650d012e5044ff0c"),
}


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=60) as r:
        return r.read()


def pcjs_disk(text: bytes) -> bytes:
    """PCjs sector JSON -> flat image: each sector's `d` is little-endian 32-bit words, the
    last word repeated to fill `l` bytes; cylinders, then heads, then sectors in order."""
    disk = json.loads(text)
    out = bytearray()
    for cylinder in disk["diskData"]:
        for head in cylinder:
            for sector in sorted(head, key=lambda s: s["s"]):
                words = sector["d"]
                n = sector["l"] // 4
                words = words + [words[-1]] * (n - len(words))
                out += b"".join(struct.pack("<i", w) for w in words)
    return bytes(out)


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    ok = True
    for name, (url, sha) in FILES.items():
        data = fetch(url)
        if name.endswith(".img"):
            data = pcjs_disk(data)
        got = hashlib.sha256(data).hexdigest()
        ok &= got == sha
        (out / name).write_bytes(data)
        print(f"{name:14} {len(data):7} bytes  {'ok' if got == sha else 'MISMATCH ' + got}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
