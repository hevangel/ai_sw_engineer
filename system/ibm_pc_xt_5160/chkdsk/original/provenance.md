# CHKDSK.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `CHKDSK.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 6,400 bytes (a `.COM`: loaded at offset 100h; code 100h-1175h, data and messages to 1979h, then zero bytes to the end of the file) |
| SHA-256 | `89f0a7bb9b2aab27acc08757c8a78c5b7179777991f9b011d575a367d9884a6c` |
| Date in the directory | 1983-03-08 12:00, first cluster 96 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img CHKDSK.COM CHKDSK.COM` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's MS-DOS 2.0 `CHKDSK.ASM`, `CHKPROC.ASM`, `CHKMES.ASM` (MIT) are kept in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); they are "Version 2.30", the binary an earlier build ([../spec/spec.md](../spec/spec.md), "The IBM binary and the source") |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
