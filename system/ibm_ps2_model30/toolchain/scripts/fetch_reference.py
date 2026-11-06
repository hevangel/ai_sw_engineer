#!/usr/bin/env python3
"""Fetch the original IBM PS/2 Model 30 (8530-002/021) BIOS and PC DOS 3.30 into a local
directory, check every SHA-256, and make the 720 KiB DOS boot disk the profile runs programs from
(spec/profile.md, "Original software"). Nothing fetched or made here may be committed.

    python3 -I system/ibm_ps2_model30/toolchain/scripts/fetch_reference.py DIR
    export PS2_ROMS=DIR

Writes:
  68X1687.BIN, 68X1627.BIN  the BIOS revision 0 (09/02/86) chips, as 86Box loads them
  ps2bios.bin               the 64 KiB BIOS at F0000h: 68X1687 on the even bytes, 68X1627 on the odd
  pcdos330.img              PC DOS 3.30 disk 1, 360 KiB (5.25-inch), converted from PCjs's sector JSON
  pcdos330_720k.img         a 720 KiB PC DOS 3.30 boot disk: the 3.30 boot record with the 720 KiB
                            BIOS parameter block, then IBMBIO.COM, IBMDOS.COM, COMMAND.COM copied from
                            disk 1 in that order (the boot record loads IBMBIO.COM from cluster 2),
                            as FORMAT A: /S would leave it. Its hash is checked too.
Standard library only, plus pcdisk (built from system/ibm_pc/platform/tools/pcdisk) for the copy.
"""

import hashlib
import json
import os
import struct
import subprocess
import sys
import urllib.request
from pathlib import Path

ROMS = ("https://raw.githubusercontent.com/86Box/roms/"
        "c4cd8da9034a8557896ff6a975ae58499a83bb80/machines/ibmps2_m30")
DISKS = ("https://raw.githubusercontent.com/jeffpar/pcjs-diskettes/"
         "cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/3.30")
SHA = {
    "68X1687.BIN": "ef2105f6f489ac4b4dcbbb1a7984aa84a207adf173328614813729a692730f6a",
    "68X1627.BIN": "6651af5f641184058e7a2b57a49e677962b278515925c157f2a5a08b51b22f46",
    "ps2bios.bin": "e3de87dd253468fc3f739f6d07f4444f58bc1b34e1ebfc088d125d5db048c6f7",
    "pcdos330.img": "f109f149d9a770f9ac3ab64e0fb03588f54fc5e8dada29722f3eaa9dc8e5af33",
    # made here from the files above, so its hash is pinned too (recorded 2026-11-06)
    "pcdos330_720k.img": "ec8965c66541e2b981fcb65c57f9d9322d0c94c5bf48b169eee207e9c7fc3404",
}


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as r:
        return r.read()


def pcjs_disk(text):
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


def main():
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    root = Path(__file__).resolve().parents[4]
    for n in ("68X1687.BIN", "68X1627.BIN"):
        (out / n).write_bytes(fetch(f"{ROMS}/{n}"))
    even, odd = (out / "68X1687.BIN").read_bytes(), (out / "68X1627.BIN").read_bytes()
    (out / "ps2bios.bin").write_bytes(bytes(b for pair in zip(even, odd) for b in pair))
    (out / "pcdos330.img").write_bytes(pcjs_disk(fetch(f"{DISKS}/PCDOS330-DISK1.json")))

    # The 720 KiB boot disk.
    pcdisk_dir = root / "system/ibm_pc/platform/tools/pcdisk"
    target = os.environ.get("CARGO_TARGET_DIR", "/tmp/pcdisk-target")
    subprocess.run(["cargo", "+1.99.1", "build", "--quiet", "--release"], cwd=pcdisk_dir, check=True,
                   env={**os.environ, "CARGO_TARGET_DIR": target})
    pcdisk = Path(target) / "release" / "pcdisk"
    disk = out / "pcdos330_720k.img"
    subprocess.run([pcdisk, "blank", disk, "720"], check=True)
    boot = bytearray((out / "pcdos330.img").read_bytes()[:512])
    blank = disk.read_bytes()
    boot[11:30] = blank[11:30]          # the 720 KiB BPB (bytes/sector .. hidden sectors)
    disk.write_bytes(bytes(boot) + blank[512:])
    for name in ("IBMBIO.COM", "IBMDOS.COM", "COMMAND.COM"):
        data = out / ("x_" + name)
        subprocess.run([pcdisk, "extract", out / "pcdos330.img", name, data], check=True)
        subprocess.run([pcdisk, "add", disk, data, name], check=True)
        data.unlink()

    ok = True
    for n in ("68X1687.BIN", "68X1627.BIN", "ps2bios.bin", "pcdos330.img", "pcdos330_720k.img"):
        got = hashlib.sha256((out / n).read_bytes()).hexdigest()
        exp = SHA[n]
        status = "ok" if got == exp else "MISMATCH " + got
        ok &= got == exp
        print(f"{n:18} {(out / n).stat().st_size:7} bytes  {status}")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
