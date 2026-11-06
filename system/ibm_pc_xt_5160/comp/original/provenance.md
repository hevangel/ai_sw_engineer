# COMP.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `COMP.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 2,523 bytes (a `.COM` image loaded at offset 0100h) |
| SHA-256 | `4b30a36d5f88ccc8d3ece2395d5043985442f44287740a4c6bc59bd99636b5b1` |
| Date in the directory | 1983-03-08 12:00 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img COMP.COM COMP.COM` |
| Author | the program's own banner (at 0A02h, never printed): "The IBM Personal Computer File Compare Utility Version 2.00 (C)Copyright IBM Corp 1981, 1982, 1983 Licensed Material - Program Property of IBM Author - David Litton" |
| License | IBM copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | none published: COMP is IBM's own program, not part of Microsoft's MS-DOS 2.0 source release; the specification is from the binary |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
