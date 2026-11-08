# DISKCOMP.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `DISKCOMP.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 2,074 bytes (a `.COM`: loaded at offset 100h; `JMP 810h` and the copyright text 100h-1ADh, state 1AEh-1C0h, code 1C1h-60Bh, messages 60Ch-80Dh, start 80Eh-919h) |
| SHA-256 | `5881d93e9f08c7df40c26c7cfa7f848558538c65639f70b2c50f10b378504f26` |
| Text in the file | `The IBM Personal Computer Diskette Compare Utility Version 2.0  (C)Copyright IBM Corp, 1982 Licensed Material - Program Property of IBM Authors - David Litton, Mark Tyson` |
| Date in the directory | 1983-03-08 12:00, first cluster 117 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img DISKCOMP.COM DISKCOMP.COM` |
| License | IBM copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | none published. Microsoft's MS-DOS source releases contain no DISKCOMP of this version; the program is IBM's, by the authors of DISKCOPY 2.0 ([../../diskcopy](../../diskcopy/original/provenance.md)) |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
