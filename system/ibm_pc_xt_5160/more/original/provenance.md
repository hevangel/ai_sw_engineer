# MORE.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `MORE.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 384 bytes (a `.COM` image loaded at offset 0100h: 231 bytes of code, the data and messages, then 102 zero bytes, the start of the 4,096-byte buffer at 021Ah) |
| SHA-256 | `fa8acd69d5a9baf560931139d8ee8fdd1acf04807b10a3286dd9e2aff99c0f16` |
| Date in the directory | 1983-03-08 12:00 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img MORE.COM MORE.COM` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's MS-DOS 2.0 `MORE.ASM` (MIT) is kept in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); with `IBMVER` it is this program except for the two instructions at 1DCh (spec S1); the messages are in `MOREMES.ASM`, not fetched (the binary's text is in [../spec/spec.md](../spec/spec.md)) |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
