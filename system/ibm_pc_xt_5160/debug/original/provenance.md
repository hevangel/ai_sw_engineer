# DEBUG.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `DEBUG.COM` on IBM PC DOS 2.00 disk 2 |
| Size | 11,904 bytes (a `.COM`: loaded at offset 100h; `JMP SHORT DSTRT` over a 9-byte `"Vers 2.30"` that is present in Microsoft's source but **absent in the IBM binary** — the IBM build jumps straight into the start) |
| SHA-256 | `3b5f8d71c125c937dd8e04760120f8b04d595a5ee32cab44f1f3e383507afcf8` |
| Disk | `pcdos200d2.img`, PC DOS 2.00 disk 2, SHA-256 `2b80a8a3554d29706444aa1a9928a32aca70b10b4d88e733650d012e5044ff0c`, fetched by `../../toolchain/scripts/fetch_reference.py`; the program is copied onto drive A by the setup item `a:disk2:DEBUG.COM` (`../../scripts/capture_dos.sh`) |
| Extraction | `pcdisk extract pcdos200d2.img DEBUG.COM DEBUG.COM` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's MS-DOS 2.0 `DEBUG.ASM`, `DEBCOM1.ASM`, `DEBCOM2.ASM`, `DEBASM.ASM`, `DEBUASM.ASM`, `DEBMES.ASM`, `DEBCONST.ASM`, `DEBDATA.ASM`, `DEBEQU.ASM` (MIT) are in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); they are built with `IBMVER` and differ from the IBM binary only in the `"Vers 2.30"` header (the README there lists the differences) |

The binary is the oracle: it runs on MartyPC's IBM 5160 under PC DOS 2.00 (disk 1 in drive A, DEBUG.COM copied from disk 2).
