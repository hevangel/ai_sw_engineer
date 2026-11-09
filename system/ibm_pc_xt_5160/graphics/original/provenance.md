# GRAPHICS.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `GRAPHICS.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 789 bytes (a `.COM`: loaded at offset 100h; resident handler 100h-334h, moved to 60h-295h; transient part 335h-37Bh; the copyright text 37Ch-414h) |
| SHA-256 | `949803e16dead03b3877ed030d7ef1ea28004e260ce20e11d4078a950691283d` |
| Text in the file | `The IBM Personal Computer Graphics Printer SupportVersion 1.00 (C)Copyright IBM Corp 1982Licensed Material - Program Property of IBMAuthor - M. Hallerman`, never shown |
| Date in the directory | 1983-03-08 12:00, first cluster 191 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img GRAPHICS.COM GRAPHICS.COM` |
| License | IBM copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | none known to be published (GRAPHICS is not in Microsoft's MIT-licensed MS-DOS 2.0 source release) |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
