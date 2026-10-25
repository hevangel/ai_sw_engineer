# Shooting Stars final report

| Check | Result |
|---|---|
| Host replay | 4/4 |
| Image size vs load area | 2475 / 14080 bytes |
| RAM | 0x09AB–0x09C4 (data and frames after the code) |
| Call depth | 3 of 7 |
| RTL replay, image in RAM | 4/4 |
| RTL replay, booted from disk | 4/4 |
| Secondary emulator | not run |

## Known gaps and limits

- DEL at the shot prompt (SS-1) is not covered by a transcript.

## Lessons

- The original assembled from its listing (with an independent assembler,
  `platform/scripts/as8008.py`) is a byte oracle: no guesswork about the
  game's text or timing of input.
