# TREE oracle

`transcripts/<case>.txt`: the original `TREE.COM` (SHA-256 `132d9995...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of
`../scripts/cases.py`: PC DOS 2.00 disk 1 in drive A, the case's blank 360 KiB disk built with `pcdisk` in
drive B (none for `-`), the date and time prompts answered with Enter, the command typed at `A>`, the 25
screen rows recorded (header lines start with `#`). Recorded with `../../scripts/capture_dos.sh`.
Cases t02-t05 and t08 print more than a screen: their command line has scrolled off (see
`../plans/testplan.md` for how they are compared).
