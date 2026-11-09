# PRINT.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `PRINT.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 4,608 bytes (a `.COM`: loaded at offset 100h; resident part 100h-BF9h (INT 27h with DX = 0BFAh), transient part BFAh-11C3h, then its tables and buffers to 12FFh, zeros) |
| SHA-256 | `a437d515bff2641cb4c0e17c8744f5151fdfca23f068b649dcd6d1db5e548113` |
| Text in the file | only its messages (spec Layout); no copyright, version or author string |
| Date in the directory | 1983-03-08 12:00, first cluster 172 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img PRINT.COM PRINT.COM` |
| License | IBM copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | none known to be published (PRINT is not in Microsoft's MIT-licensed MS-DOS 2.0 source release) |

The binary is the oracle: it runs on 86Box's IBM PC/XT with a text printer on LPT1, under the same PC DOS 2.00 disk.
