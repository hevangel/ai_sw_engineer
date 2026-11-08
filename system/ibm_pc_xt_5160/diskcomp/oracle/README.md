# DISKCOMP oracle

`transcripts/<case>.txt`: the original `DISKCOMP.COM` (SHA-256 `5881d93e...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS, two double-sided drives), one boot per case of
`../scripts/cases.py`: in drive A PC DOS 2.00 disk 1, or for `a:system360` its double-sided 360 KiB copy made by
`pcdisk syscopy`; in drive B (none for `-`) the case's disk: for `b:copy_a` a copy of that drive A (the program
under test included), then pcdisk's `b:` steps, or for `b:image:NAME` the raw image cases.py writes; both writable;
the date and time prompts answered with Enter, the command typed at `A>`, each key typed once its prompt is on the
screen (a wait text starting with `$`: once the last non-blank row ends with it, the screen having changed), the
25 screen rows recorded (header lines start with `#`, screen rows with `|`). DISKCOMP only reads, so there is no
drive B hash (`SAVE_B` is not used). Recorded with `../../scripts/capture_dos.sh`; the longer cases need
`MARTY_CYCLES=1500000000`. The disks of the capture stay in `build/oracle/` (uncommitted: they hold IBM's files).
