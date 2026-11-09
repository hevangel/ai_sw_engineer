# EDLIN oracle

`transcripts/<case>.txt`: the original `EDLIN.COM` (SHA-256 `d4ea0212...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of `../scripts/cases.py`:
in drive A PC DOS 2.00 disk 1 with the case's files added by `pcdisk add`, writable; no drive B; the date and time
prompts answered with Enter, the command typed at `A>`, then each key sequence typed once its prompt is on the
screen (a wait text starting with `$`: once the last non-blank row ends with the rest, the screen having changed
since the previous wait: `$*` the command and insert prompts, `$?` `O.K.?` and `Abort edit (Y/N)?`, `$A>` COMMAND's
prompt), the 25 screen rows recorded (header lines start with `#`, screen rows with `|`). `^Z` and `^C` in the keys
are typed with Ctrl held. Each editing case ends with E or Q and then TYPEs the file(s) at the next `A>`, so the
screen shows what EDLIN wrote. Recorded with `../../scripts/capture_dos.sh` (default cycle budget). The disks of the
capture stay in `build/oracle/` (uncommitted: they hold IBM's files; MartyPC does not write them back, so they are
the disks as built, before the run).
