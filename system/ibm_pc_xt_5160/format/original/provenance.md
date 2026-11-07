# FORMAT.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `FORMAT.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 6,016 bytes (a `.COM`: loaded at offset 100h; code 100h-0C1Bh and 0E94h-1241h, data and messages, the BPB table at 12FAh, two boot records at 1450h and 1650h; the file ends at 187Fh) |
| SHA-256 | `cd86654e65be54a851818d0075327e902a1b69fd08a1ac3955d37b253ff7b9b2` |
| Date in the directory | 1983-03-08 12:00, first cluster 84 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img FORMAT.COM FORMAT.COM` |
| License | IBM / Microsoft copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | Microsoft's generic MS-DOS 2.0 `FORMAT.ASM`, `FORMES.ASM`, `GENFOR.ASM` (MIT) are kept in [../reference/ms-dos-2.0/](../reference/ms-dos-2.0/README.md); IBM's OEM module is not published |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk. The disks it writes hold
IBM's boot record (and, with /S, IBMBIO.COM, IBMDOS.COM and COMMAND.COM): they stay in `build/` (gitignored) and only
their SHA-256 is committed (`../oracle/transcripts/`, `../oracle/drive_b_masked.txt`).
