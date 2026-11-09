# ASSIGN oracle

`transcripts/<case>.txt`: the original `ASSIGN.COM` (SHA-256 `080db454...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of `../scripts/cases.py`:
in drive A PC DOS 2.00 disk 1 with `ATEXT.TXT` added by `pcdisk add`, in drive B a blank 360 KiB disk holding
`ONLY.TXT` (`b:add:ONLY.TXT=ONLY.TXT`), both writable; the date and time prompts answered with Enter, the case's
command typed at `A>`, then each further command line typed once the last non-blank row ends with `A>` and the
screen has changed (`$A>`); the 25 screen rows recorded (header lines start with `#`, screen rows with `|`).
Recorded with `../../scripts/capture_dos.sh` (default cycle budget). In `s01_a_to_b` the boot's date and time rows
have scrolled off; every transcript still shows its first command line. The disks of the capture stay in
`build/oracle/` (uncommitted: drive A holds IBM's files).
