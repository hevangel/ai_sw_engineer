# SORT.EXE of PC DOS 2.00: provenance

| | |
|---|---|
| File | `SORT.EXE` on IBM PC DOS 2.00 disk 1 |
| Size | 1,280 bytes (MZ header of 512 bytes, load module of 768) |
| SHA-256 | `755a3c1cad3769ff45af807eff32eb76012810e565f88ccdf8a4d7085836528a` |
| Date in the directory | 1983-03-08 12:00 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img SORT.EXE SORT.EXE` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's MS-DOS 2.0 `SORT.ASM` (MIT, by Chris Peters) is kept in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); it is a later, different version (collating table, 3-18-83 fixes), not the source of this binary |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
