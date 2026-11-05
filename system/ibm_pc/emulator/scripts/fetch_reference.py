#!/usr/bin/env python3
"""Fetch the original IBM 5150 firmware, character ROM and PC DOS 1.00 disk
from the pinned PCjs revisions into a local directory, convert them to flat
bytes, write the blank work disk, and check every SHA-256 against
original/provenance.md. Nothing fetched here may be committed.

    system/ibm_pc/emulator/scripts/fetch_reference.py DIR
    export IBM_PC_ROMS=DIR

Run with python3 -I. Standard library only.
"""

import hashlib
import json
import struct
import sys
import urllib.request
from pathlib import Path

PCJS = "https://raw.githubusercontent.com/jeffpar/pcjs/1a91d38eb12e5fdf91116b21ad9ef1935710a496"
DISKS = (
    "https://raw.githubusercontent.com/jeffpar/pcjs-diskettes/"
    "cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/1.00"
)

FILES = {
    "bios.bin": (
        PCJS + "/machines/pcx86/ibm/5150/rom/bios/1981-04-24/PCBIOS-REV1.rom",
        "a259306c47b43e14ef6378d7bdeff67331a2519d363bc36b3f354bc91a6874c8",
    ),
    "basic.bin": (
        PCJS + "/machines/pcx86/ibm/5150/rom/basic/BASIC100.rom",
        "2452b03b4b724b5b81e8cc367809c521144f5fe4e0425caee42ac71240b9b102",
    ),
    "cga.bin": (
        PCJS + "/machines/pcx86/ibm/video/cga/IBM-CGA.rom",
        "37527f661580e5a09710051cec67422abfadf31c61b841537a91e7f27be19304",
    ),
    "pcdos100.img": (
        DISKS + "/PCDOS100.json",
        "29a4c5eb355b2bfb1dbbece96d91eabc5c1331c6299437d00dd8d862ac126eb2",
    ),
}
WORKDISK_SHA256 = "2404d6897ea480b9b479928151de58d1bdadb7e7d4bc36210766189037e43018"


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=60) as r:
        return r.read()


def pcjs_disk(text: bytes) -> bytes:
    """PCjs sector JSON -> flat image: each sector's `d` is little-endian
    32-bit words, the last word repeated to fill `l` bytes."""
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


def workdisk() -> bytes:
    """A formatted blank 160 KiB data disk: boot sector and root directory
    zero, both FATs FE FF FF, data sectors F6h."""
    b = bytearray([0xF6]) * 163840
    b[0 : 7 * 512] = bytes(7 * 512)
    for s in (1, 2):
        b[s * 512 : s * 512 + 3] = b"\xfe\xff\xff"
    return bytes(b)


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
        status = "ok" if got == sha else f"MISMATCH (got {got})"
        ok &= got == sha
        (out / name).write_bytes(data)
        print(f"{name:14} {len(data):7} bytes  {status}")
    w = workdisk()
    assert hashlib.sha256(w).hexdigest() == WORKDISK_SHA256
    (out / "workdisk.img").write_bytes(w)
    print(f"{'workdisk.img':14} {len(w):7} bytes  ok (generated)")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
