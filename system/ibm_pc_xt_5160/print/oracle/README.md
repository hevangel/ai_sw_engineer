# PRINT oracle

`transcripts/<case>.txt`: the original `PRINT.COM` (SHA-256 `a437d515...`) run by the original PC DOS 2.00 on
86Box `a1e92633`'s IBM PC/XT (`ibmxt`, BIOS `ibm5160_1501512_5000027`: the 1982-11-08 XT BIOS and Cassette BASIC
C1.10, the same bytes as MartyPC's; 8088 at 4.77 MHz, 256 KiB, CGA, the clock not synchronised with the host) with
LPT1 the Generic Text Printer, one boot per case of `../scripts/cases.py`: in drive A PC DOS 2.00 disk 1 with the
case's files added by `pcdisk add`; the date and time prompts answered with Enter, the command typed at `A>`, the
list-device question answered once `Name of list device` is on the screen; then the machine runs until the screen
has not changed for 4 s of machine time and 6 s more, so the printer's 5-second page time-out has written the last
page. The 25 screen rows are recorded (header lines start with `#`, screen rows with `|`), then each line of the
page files the printer wrote (`#lpt|`, the files split on `\n`, a form feed shown as `<FF>`). Recorded with
`../../scripts/capture_dos.sh` with `ORACLE=86box` (`../../scripts/xt_86box.py`). MartyPC is not used: it keeps its
parallel port private. The disks of the capture stay in `build/oracle/` (uncommitted: they hold IBM's files).
