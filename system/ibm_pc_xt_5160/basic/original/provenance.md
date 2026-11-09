# IBM PC BASIC for the XT: provenance of the originals

Neither binary is committed (IBM / Microsoft copyright, not redistributable; `original/*` is gitignored).
Both are fetched by [`../../toolchain/scripts/fetch_reference.py`](../../toolchain/scripts/fetch_reference.py)
into `XT_ROMS`. Together they are the oracle: BASIC.COM runs only with the ROM present.

## Cassette BASIC C1.10 (ROM)

| | |
|---|---|
| File | `xtbasic.bin` (the four BASIC ROM chips of the IBM 5160 as one 32 KiB image, F600:0000-F600:7FFF) |
| Size | 32,768 bytes |
| SHA-256 | `3033d1a54c99d7e2aa1fc7c8c2e51a56ae1b61bf4e70e8aa580f43c43e37a63e` |
| Source URL | [pcjs `machines/pcx86/ibm/5160/rom/basic/BASIC110.rom`](https://raw.githubusercontent.com/jeffpar/pcjs/1a91d38eb12e5fdf91116b21ad9ef1935710a496/machines/pcx86/ibm/5160/rom/basic/BASIC110.rom) at commit `1a91d38e` |
| Also in | the 86Box ROM set's two XT chips: `BIOS_5160_08NOV82_U19_5000027.BIN` (BASIC 0000h-1FFFh) and `BIOS_5160_08NOV82_U18_1501512.BIN` (BASIC 2000h-7FFFh + BIOS), [86Box/roms `c4cd8da9`](https://github.com/86Box/roms/tree/c4cd8da9/machines/ibmxt) |
| Text in the ROM | `The IBM Personal Computer Basic` / `Version C1.10 Copyright IBM Corp 1981` (offset 4C9Bh), then the date `25-Apr-81` |
| Year, authors | 1981; Microsoft for IBM (the copyright line names IBM; Microsoft's authorship from the GW-BASIC code base, [../reference/README.md](../reference/README.md)) |
| License | IBM copyright; not redistributable |

## Disk BASIC D2.00 (BASIC.COM)

| | |
|---|---|
| File | `BASIC.COM` on IBM PC DOS 2.00 disk 1 |
| Size | 16,256 bytes (a `.COM`: `JMP 39E6h` at 100h) |
| SHA-256 | `c08624b8cd41724cb64f81a378094208e65e05deccfa6798aa95dd403aaffad5` |
| Date in the directory | 1983-03-08 12:00, first cluster 209 |
| Text in the file | `The IBM Personal Computer Basic` / `Version D2.00 Copyright IBM Corp. 1981, 1982, 1983`, `17-Jan-83`, `Licensed Material - Program Property of IBM` |
| Disk | `pcdos200.img`, PC DOS 2.00 disk 1, SHA-256 `934e62ceb797537600847902e627908969bd99df6a58637c4045e1ecaeb53a88`, from [pcjs-diskettes](https://github.com/jeffpar/pcjs-diskettes/tree/cd3433748b09837a4c60ca7066f3642772b2d711/pcx86/sys/dos/ibm/2.00) at `cd343374` |
| Extraction | `pcdisk extract pcdos200.img BASIC.COM BASIC.COM` |
| Year, authors | 1983 (PC DOS 2.00, released with the XT); Microsoft for IBM |
| License | IBM / Microsoft copyright; not redistributable |
| Not in scope | `BASICA.COM` (Advanced BASIC A2.00, 25,984 bytes, SHA-256 `556821bba6f3990f9cd95a0f40d39788738a20bf359475e84bf7cb29b999b197`) on the same disk; a separate title |

## The oracle run

MartyPC (`05c0d088`) IBM 5160, 256 KiB, XT BIOS 1982-11-08 (`xtbios.bin`, SHA-256
`2798f8a9a7dcab651ede40373d35269ad573b75acf811115c263e034ef80a1f2`) and `xtbasic.bin`, booting a copy of
PC DOS 2.00 disk 1 with the case's files; `system/ibm_pc_xt_5160/scripts/capture_dos.sh`.
