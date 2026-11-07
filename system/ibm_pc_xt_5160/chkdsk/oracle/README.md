# CHKDSK oracle

`transcripts/<case>.txt`: the original `CHKDSK.COM` (SHA-256 `89f0a7bb...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of `../scripts/cases.py`:
PC DOS 2.00 disk 1 in drive A, the case's 360 KiB disk in drive B (the damaged image cases.py writes, or a blank
disk built with `pcdisk`; none for `-`), both writable, the date and time prompts answered with Enter, the command
typed at `A>`, each prompt's keys typed once its text is on the screen (the Y/N answers with Enter; c08 types its
second command at the `A>` after the first; c15 types `b:`, then `a:chkdsk` and `a:` at the `B>` prompts), the 25 screen rows recorded (header lines start with `#`, screen rows
with `|`). Recorded with `../../scripts/capture_dos.sh`. Cases c03, c05, c06, c07, c10 and c11 scroll part of the
boot lines off the screen; c08 scrolls its first command line off (see `../plans/testplan.md` for how it is
compared).
