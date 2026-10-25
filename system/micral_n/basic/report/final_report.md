# SCELBAL final report

| Check | Result |
|---|---|
| Host replay (transcripts identical) | 10/10 |
| Target image size vs load area | 13826 / 14080 bytes (code 10691, the RAM image of pages 01, 26, 27 and 9 program pages, frames) |
| RAM | everything in the image except the array page (0x3700–0x37FF) |
| Call depth | 6 of 7 |
| RTL replay, image in RAM | 10/10 |
| RTL replay, booted from disk by the Rust boot ROM | 10/10 |
| Secondary emulator | not run (test plan) |

Speed on the RTL, cycles for the same keys (including the final wait):

| Transcript | Original | Rust | Ratio |
|---|---|---|---|
| 01_print_numbers | 2 374 466 | 3 903 596 | 1.6 |
| 04_loops_subroutines | 1 639 800 | 4 269 991 | 2.6 |
| 07_arrays | 1 122 742 | 2 643 970 | 2.4 |
| 09_program | 15 797 551 | 44 504 015 | 2.8 |

## Known gaps and limits

- The program buffer is 2304 bytes, half the listing's (spec BAS-4). The
  original's 12 KiB layout would need about 2.3 KB less code.
- Before the first `SCR` the original corrupts memory; the Rust does not
  reproduce that (BAS-2). Reads past SCELBAL's data read 0, not the
  original's code bytes (BAS-3).
- The five carry loops are assembly on the target (`asm/fp.s`); the host
  runs their Rust versions, so the asm is checked by the RTL replay only.

## Lessons

- The oracle found a backend defect the backend's own tests could not:
  `memset`'s value register (`failure_notes/2026-10-24-i8008-memset-value-register.md`).
- On the 8008 the error path decides the size. SCELBAL's `JMP EXEC` from any
  depth is free because the hardware stack is circular; modelling it with
  `Result` cost 40% of the image. A linker `RESET` point makes the
  original's control flow checkable instead.
- A compiled 8-bit carry chain costs ten times the hand-written loop with
  four allocatable registers. Keeping those primitives in asm, with the
  Rust version as the contract and the host implementation, is the cheap fix.
