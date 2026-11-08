# DEBUG oracle

`transcripts/<case>.txt`: the original `DEBUG.COM` (SHA-256 `3b5f8d71...`) run by the original
PC DOS 2.00 on MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per
case of `../scripts/cases.py`: in drive A PC DOS 2.00 disk 1 with DEBUG.COM copied from disk 2
(`a:disk2:DEBUG.COM`) and the case's files added, writable; no drive B; the date and time prompts
answered with Enter, the command typed at `A>`, then each key sequence typed once its prompt is on
the screen (a wait text starting with `$`: once the last non-blank row ends with the rest, the
screen having changed since the previous wait: `$-` DEBUG's `-` prompt, `$:` the `R <reg>` value
prompt, `$` A's address prompt (any screen change), `$A>` COMMAND's prompt), the 25 screen rows
recorded (header lines start with `#`, screen rows with `|`). Each case ends with Q; a case that
writes a file then TYPEs it, so the screen shows what DEBUG wrote. Recorded with
`../../scripts/capture_dos.sh` (default cycle budget). The disks of the capture stay in
`build/oracle/` (uncommitted: they hold IBM's files).
