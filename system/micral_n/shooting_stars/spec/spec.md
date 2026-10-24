# Shooting Stars — behavior spec

Original: "Shooting Stars", Willard I. Nico (Delta t, Houston), *BYTE*, May
1976 (cover story), a game for the SCELBI 8H (8008). MO5 ported it to the
Micral N in January 2022 and ran it on their machine in March 2022 (HxC
restoration timeline). Source used here: the listing as transcribed for the
AS assembler by Jim Loos (`original/fetch.sh`, SHA-256 in
`original/shooter.asm.sha256`; repository MIT-licensed), which keeps the
program and replaces only the SCELBI terminal routines.

The oracle is the original program itself: `scripts/build_original.sh`
assembles the listing (with `platform/scripts/as8008.py`) and swaps in Micral N
teletype routines for RST 6 and RST 7 (`oracle/micral_io.asm`).
`scripts/capture_oracle.py` runs it on the ai_hw_engineer RTL and records the
transcripts in `oracle/transcripts/`.

## Game

The universe is a 3×3 field. Position 5 is the centre; the other eight make
up the fringe. Each holds a star `*` or a black hole `.`. The game starts
with only the centre a star. Typing a star's number (1–9) "shoots" it: every
position in that star's galaxy (the MASKTAB of the listing) flips between
star and black hole. Shooting a black hole is refused. The player wins with
every fringe position a star and the centre a black hole. The player loses
when all nine are black holes. The best score is 11 shots.

## Terminal behavior (all from the listing)

- Prints a CR, the heading, and "WANT THE RULES? ". The answer is read with
  echo up to CR. Unless its first key is `N`, three rule pages follow, each
  ended by any key (read without echo).
- Each round: three CRs, then the universe repeatedly. Each position prints
  `*` or `.` and two spaces; two CRs separate the rows. Then five CRs,
  "YOUR SHOT? ", one key echoed, three CRs.
- An invalid key prints "THAT WASNT A VALID STAR NUMBER!" and a black hole
  prints "HEY! YOU CAN ONLY SHOOT STARS, …". Neither counts as a shot.
- On a win, "YOU WIN! GOOD SHOOTING: … YOU FIRED  n SHOTS. …" (n without
  leading zeros), then "WANT TO SHOOT AGAIN DEADEYE?  ". On a loss, "YOU LOST
  THE GAME: …". The answer is read with echo up to CR: `Y` plays again,
  anything else halts.
- Messages use LF as the line separator; the game's own breaks are CRs.

## Assumption ledger

| ID | Assumption | Why |
|---|---|---|
| SS-1 | DEL at "YOUR SHOT?" prints the intended "YOU GIVE UP TOO EASILY! …" | the listing loads the message pointer with `LLI` twice (no `LHI`), so the original prints from an address that depends on its layout; no transcript covers DEL |
| SS-2 | The teletype is 7-bit, keys arrive uppercase | the listing compares with `'N'`, `'Y'` and the digits; the Micral N teletype driver masks bit 7 (platform IO-2) |
