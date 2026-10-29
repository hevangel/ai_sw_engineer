# ibm_pc emulator oracle

Transcripts recorded from the hardware project's IBM PC RTL (`$AI_HW_ENGINEER/system/ibm_pc`,
the whole 5150 running the unchanged IBM BIOS, Cassette BASIC and PC DOS 1.00), used to
check the emulator. The format is the session format in `../src/session.rs`: `boot`,
`type <text>`, `until <contains|endswith|row> <text>`, then the settled 25-row screen
(`|` rows), in graphics modes the SHA-256 of video memory with the CGA mode and colour registers,
and for drive B the SHA-256 of its image.

- `transcripts/dos100_session.txt`: boot to the date prompt, date entry, `DIR`, `CHKDSK`,
  `DATE`, `BASIC`, `print 6*7`, `SYSTEM`, `FORMAT B:`, `COPY TIME.COM B:CLOCK.COM`, `DIR B:`, then ROM
  BASIC graphics (`SCREEN 1,0`, a filled box, a circle, `SCREEN 2`, a line) with video memory fingerprints.
  Recorded by `../scripts/rtl_oracle.sh` (about ten minutes); do not edit by hand.

Never write expected output from this emulator's own results: re-record from the RTL, or
leave the transcript alone. The transcripts contain only text the original programs
displayed (prompts, file names and sizes, CHKDSK totals), not the programs themselves.
