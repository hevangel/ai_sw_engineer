# FIND oracle

`transcripts/<case>.txt`: the original `FIND.EXE` (SHA-256 `947b34cb...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of
`../scripts/cases.py`: the case's input files added to PC DOS 2.00 disk 1 with `pcdisk`, drive A writable, the date
and time prompts answered with Enter, the command typed at `A>`, the 25 screen rows recorded (header lines start
with `#`, screen rows with `|`). Recorded 2026-11-06 with `../../scripts/capture_dos.sh`. MartyPC shows screen bytes
outside 20h-7Fh as spaces.
