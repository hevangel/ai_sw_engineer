# SCELBAL implementation plan

1. Fetch the listing; record provenance (done).
2. Build the oracle: the listing's bytes with the Micral N teletype
   (`scripts/build_original.py`); capture transcripts on the RTL (done, 10).
3. Write `spec/spec.md` and `spec/io.md` (done).
4. Rust interpreter over the original's memory layout; host replay (done).
5. Target image through llvm-i8008; fit the 7-level stack and the load
   area (done: depth 6, 13826 B, 10 program pages).
6. RTL replay, direct and booted (done, 10/10 each).
7. Report (done); size history.

## Design decisions

- **A transliteration, not a redesign.** SCELBAL's behaviour (its number
  formats, error codes, and the quirks of a parser that keeps its state in
  fixed memory) follows from its routines and its memory layout. The Rust
  keeps both: one function per listing routine where that fits, the same
  page/offset data, the same byte-level floating point. Only control flow
  is restructured (loops instead of jumps, `Go` for the executive's targets).
- **Errors restart the executive** as the original's `JMP EXEC` does; on
  the target that is a jump from any depth (spec "Target"). Threading a
  `Result` through every routine made the image 40% larger.
- **Size.** In order of effect: non-returning errors; a page-aligned RAM so
  that `page[offset]` is `H = page, L = offset` (llvm-i8008 known bits for
  aligned globals); one call site each for the parser and the operators;
  the power-on image as initialised data instead of code; asm for the five
  carry loops; the array page moved to the boot ROM's buffer; later
  backend work (page-indexed accesses, branch-free compares). 15.8 KB of
  code became 10.5 KB.
- **Not reproduced**: what the original does before the first `SCR`
  (spec BAS-2) and reads of its own code bytes (BAS-3); the program buffer
  is 10 pages instead of 18 (BAS-4).
