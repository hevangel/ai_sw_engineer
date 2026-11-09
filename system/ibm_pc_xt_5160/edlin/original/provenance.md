# EDLIN.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `EDLIN.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 4,608 bytes (a `.COM`: loaded at offset 100h; `JMP 108h`, code 102h-E58h, IBM's copyright text E5Ah-ED0h, constants and messages ED1h-1293h, then 44 bytes of text left over from another build (`$`, CR LF, `Enter new date: $`, CR LF, `Invalid time$Current t`, 1294h-12BFh) and 64 zeros, which are the initial values of the variables there) |
| SHA-256 | `d4ea02122e53dfef7107288f30515084efbcadacc36fcba10202aab64d936c4c` |
| Text in the file | `The IBM Personal Computer EDITOR Version 2.00 (C)Copyright IBM Corp 1981,1982 Licensed Material - Program Property of IBM`, then `BAK***MAUlloa/Microsoft/V20***12` (Microsoft's revision 12) |
| Date in the directory | 1983-03-08 12:00, first cluster 127 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img EDLIN.COM EDLIN.COM` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's MS-DOS 2.0 `EDLIN.ASM`, `EDLPROC.ASM`, `EDLMES.ASM` (MIT, by M. A. Ulloa, N. Panners, A. Reynolds) are kept in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); they are revision 14 (7/23/83), the binary revision 12, and differ from it in the places listed there. Microsoft's own build in the same repository, `v2.0/bin/EDLIN.COM` (4,389 bytes, SHA-256 `799767e5bc6727fd3fb1b0c54a2099c5c53d46dcab5b7b5012b688f3c718b444`), is revision 12 too |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
