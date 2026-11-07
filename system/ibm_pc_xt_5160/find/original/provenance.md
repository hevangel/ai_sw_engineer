# FIND.EXE of PC DOS 2.00: provenance

| | |
|---|---|
| File | `FIND.EXE` on IBM PC DOS 2.00 disk 1 |
| Size | 5,888 bytes (MZ header of 512 bytes; the header gives 5,796 bytes of image, so a load module of 5,284 bytes, then 92 zero bytes DOS does not load) |
| SHA-256 | `947b34cbfe2165b497ce2ca6d52966261f35941b7ebf7a5eb3e7dff0c81d3e0b` |
| Entry | CS:IP = 0000:002Ah, SS:SP = 014Bh:0100h, no relocations |
| Date in the directory | 1983-03-08 12:00 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img FIND.EXE FIND.EXE` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's MS-DOS 2.0 `FIND.ASM` (MIT, by M. A. Ulloa) is kept in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); it is Rev. 8 (04/14/83), the binary Rev. 6 (11/10/82); Microsoft's built `FIND.EXE` has the same load module as this file |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
