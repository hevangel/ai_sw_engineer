# Shooting Stars implementation plan

1. Fetch the listing; provenance (done).
2. Oracle: the listing assembled by `platform/scripts/as8008.py` with the
   Micral N teletype; 4 transcripts on the RTL (done).
3. Spec (done). 4. Rust game; host replay (done). 5. Target image (done,
   2475 B). 6. RTL replay, direct and booted (done). 7. Report (done).

## Design decisions

- The game state (the 3×3 field, the shot count) in `u8`s and a nine-entry
  mask table, as the listing keeps them; the messages as byte strings with
  the listing's CR and LF.
