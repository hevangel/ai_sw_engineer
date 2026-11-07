# DISKCOPY.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `DISKCOPY.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 2,444 bytes (a `.COM`: loaded at offset 100h; copyright text 103h-1A1h, state 1A2h-1D1h, code 1D2h-6CBh, messages 6CCh-99Fh, start 9A0h-A8Bh) |
| SHA-256 | `2189a014867e13d645ada7abd0452f7c7ffdae71a71b5d31b96c430e0160ea08` |
| Text in the file | `The IBM Personal Computer Diskette Copier Version 2.0 (c)Copyright IBM Corp 1982 Licensed Material - Program Property of IBM Authors- David Litton, Mark Tyson` |
| Date in the directory | 1983-03-08 12:00, first cluster 112 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img DISKCOPY.COM DISKCOPY.COM` |
| License | IBM copyright; not redistributable; never committed (`original/*` is gitignored), nor any disk image it wrote |
| Source code | none published. Microsoft's MS-DOS 2.0 `DISKCOPY.ASM` (MIT, in Microsoft's MS-DOS repository) is a different program by Chris Peters (INT 25h/26h, other prompts) and is not a source of this one |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
