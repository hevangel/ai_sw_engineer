# GRAPHICS oracle

`transcripts/<case>.txt`: the original `GRAPHICS.COM` (SHA-256 `949803e1...`) run by the original PC DOS 2.00 on MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of `../scripts/cases.py`: in drive A PC DOS 2.00 disk 1 with this repository's capture tools added by `pcdisk add` (PRNCAP.COM `c998fe7f...`, PRNDUMP.COM `9576e0f6...`, DRAW.COM `dd30ec2d...`, [../tools/README.md](../tools/README.md)), in drive B a blank 360 KiB disk labelled `PRINTOUT`, both writable; the date and time prompts answered with Enter, the case's first command typed at `A>`, then each further command line typed once the last non-blank row ends with `A>` and the screen has changed (`$A>`); the 25 screen rows recorded (header lines start with `#`, screen rows with `|`).

Recorded on 2026-11-09 with `../scripts/capture.sh` (default cycle budget), which runs `../../scripts/capture_dos.sh` with `SAVE_B=1` (the `# drive B after:` line: the SHA-256 of the whole image, which holds the file's directory entry and FAT too), then extracts `OUT.PRN` from the saved image and adds `# printed: <length> bytes sha256 <hex>`: the bytes GRAPHICS (or the BIOS) sent to INT 17h for printer 0, as PRNCAP kept them. The printed bytes themselves and the disks stay in `build/oracle/` (uncommitted); `../scripts/decode.py` shows them as printer lines (spec "What the cases printed").

| Case | Printed | Status |
|---|---|---|
| g01_mode6, g08_twice | 39,972 bytes `fdf0b91f...`: 82 lines, 77 bit images | 00h |
| g02_mode4, g03_mode5 | 26,896 bytes `f04ee971...`: 52 lines, 40 bit images | 00h |
| g04_text80, g11_text80_bios | 2,052 bytes `14a8451d...`: the BIOS's text print | 00h |
| g05_text40 | 1,052 bytes `ac893df1...` | 00h |
| g06_mode6_bios | 2,052 bytes `58e4d994...`: the BIOS's print of mode 6 | 00h |
| g07_mode4_bios | 1,052 bytes `5eafc98b...` | 00h |
| g09_busy | 0 bytes | 01h |
| g10_printer_error | 1 byte (`0A`) | FFh |

Re-capture only to replace the oracle: `XT_ROMS=DIR MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR scripts/capture.sh`.
