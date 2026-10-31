# ibm_pc_xt_5160 memory map

Real-mode 20-bit physical map of the 256 KiB XT profile. Authority: IBM
April 1983 5160 Technical Reference via
`ai_hw_engineer/system/ibm_pc_xt_5160/spec/spec.md` (RAM/ROM/CGA/option-ROM
rows) and `ai_hw_engineer/system/ibm_pc/spec/reference/README.md` (BIOS,
BASIC, DOS provenance). Our oracles reproduce this map.

| Start | End | Size | Kind | Use |
|---|---|---|---|---|
| 0x00000 | 0x003FF | 1 KiB | RAM | interrupt vector table (256 far vectors) |
| 0x00400 | 0x004FF | 256 B | RAM | BIOS data area |
| 0x00500 | 0x005FF | 256 B | RAM | DOS/BIOS scratch; `.COM` free low memory starts here in practice |
| 0x00600 | 0x06FFF | ~27 KiB | RAM | DOS + user programs (DOS 2.00 resident size varies; measure per boot) |
| 0x07000 | 0x3FFFF | ~228 KiB | RAM | user programs (conventional memory; top depends on DOS resident size) |
| 0x40000 | 0x9FFFF | 384 KiB | — | unpopulated on the 256 KiB XT (no RAM; reads undefined) |
| 0xA0000 | 0xAFFFF | 64 KiB | — | reserved (EGA/VGA on later machines; absent here) |
| 0xB8000 | 0xBFFFF | 32 KiB | I/O mem | CGA text/graphics memory (aliases per CGA decoding) |
| 0xC8000 | 0xC9FFF | 8 KiB | ROM | IBM/Xebec fixed-disk option ROM (installs INT 13h) |
| 0xCA000 | 0xF5FFF | — | — | reserved for other option ROMs |
| 0xF6000 | 0xFDFFF | 32 KiB | ROM | Cassette BASIC 1.10 |
| 0xFE000 | 0xFFFFF | 8 KiB | ROM | XT BIOS (1982-11-08); reset entry at FFFF0h |

ROM writes are ignored. Conventional-memory top for a title is whatever DOS
reports (INT 12h / BIOS data area 0x413) after boot, not a fixed number.

## Budgets

- Boot-sector image: 510 code bytes at 0000:7C00 + `55 AA`; stack wherever the sector sets it (conventionally below 7C00).
- `.COM` image: one 64 KiB segment minus PSP (256 B), minus stack; code+data+stack ≤ ~63 KiB.
- MZ `.EXE` (later): headers + segments within conventional memory as DOS loads them.
- RAM available to a title (incl. stack): conventional memory minus DOS/BIOS residents — measure on the booted oracle, never assume.

Source for every row: IBM 5160 Technical Reference (Apr 1983) via the
ai_hw_engineer XT spec above; DOS-resident rows are measured per boot.
