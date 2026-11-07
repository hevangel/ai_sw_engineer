# FORMAT oracle

`transcripts/<case>.txt`: the original `FORMAT.COM` (SHA-256 `cd86654e...`) run by the original PC DOS 2.00 on
MartyPC `05c0d088`'s IBM 5160 (256 KiB, CGA, the 1982-11-08 XT BIOS), one boot per case of `../scripts/cases.py`:
PC DOS 2.00 disk 1 in drive A, the case's disk in drive B (a zero-filled 360 KiB or 180 KiB image cases.py writes,
or a 360 KiB disk with files built with `pcdisk`; none for `-`), both writable, the date and time prompts answered
with Enter, the command typed at `A>`, each prompt's keys typed once its text is on the screen (the Y/N answers and
the `strike any key` key are single keys; the volume label is typed with Enter; a `$` wait needs the last non-blank
row to end with the text, once the screen changed since the previous match), the 25 screen rows recorded (header
lines start with `#`, screen rows with `|`). With `SAVE_B=1`, `# drive B after: sha256 <hex>` is the SHA-256 of
drive B's raw image after the run (the images stay in `../build/oracle/`, not committed: they hold IBM's boot record
and, with /S, IBM's system files). Recorded with `../../scripts/capture_dos.sh`. f09 and f10 scroll the first boot
lines off the screen.

`drive_b_masked.txt`: the SHA-256 of the same images with the masks of `../plans/testplan.md` (the volume label's
time word zeroed; for /8 without /S, sector 0 zeroed), derived from `../build/oracle/<case>_b_after.img`.
