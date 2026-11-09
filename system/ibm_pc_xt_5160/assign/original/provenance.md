# ASSIGN.COM of PC DOS 2.00: provenance

| | |
|---|---|
| File | `ASSIGN.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 896 bytes (a `.COM`: loaded at offset 100h; resident part 100h-30Fh, transient part 303h-434h, then 11 bytes of build leftovers `d1.COM091.C` and zeros) |
| SHA-256 | `080db45404faefa9e071af2dd3cb4054541601673df35137622af845b3077b86` |
| Text in the file | only its messages: `Incorrect DOS version`, `Invalid parameter`; no copyright or version string |
| Date in the directory | 1983-03-08 12:00, first cluster 186 |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, fetched by `../../toolchain/scripts/fetch_reference.py` from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img ASSIGN.COM ASSIGN.COM` |
| License | IBM copyright; not redistributable; never committed (`original/*` is gitignored) |
| Source code | none known to be published (ASSIGN is not in Microsoft's MIT-licensed MS-DOS 2.0 source release) |

The binary is the oracle: it runs on MartyPC's IBM 5160 under the same PC DOS 2.00 disk.
