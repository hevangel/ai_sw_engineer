# DISKCOPY oracle

`transcripts/<case>.txt`: the original `DISKCOPY.COM` (SHA-256 `2189a014...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS, two double-sided drives), one boot per case of
`../scripts/cases.py`: in drive A PC DOS 2.00 disk 1, or for `a:system360` (the A-to-B cases) its double-sided
360 KiB copy made by `pcdisk syscopy`, plus the case's files; the case's 360 KiB disk in drive B (none for `-`);
both writable; the date and time prompts answered with Enter, the command typed at `A>`, each key typed once its
prompt is on the screen, the 25 screen rows recorded (header lines start with `#`, screen rows with `|`).
`# drive B after: sha256` is the SHA-256 of drive B's image as the run left it (`SAVE_B=1`; the images stay in
`build/oracle/`, uncommitted, since they hold IBM's files). Recorded with `../../scripts/capture_dos.sh` (d05 with
`MARTY_CYCLES=1500000000`). An earlier capture copied the one-sided disk 1 itself (40 head-1 read errors, spec
"Head 1 of a one-sided image"); it was replaced on 2026-11-07.
