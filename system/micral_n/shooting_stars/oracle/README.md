# Shooting Stars oracle

Expected behaviour captured from the **original** program on the
ai_hw_engineer 8008 RTL (Micral N harness, `platform/sim`). Never derived
from this repository's Rust.

```text
oracle/
├── micral_io.asm  — Micral N teletype routines for RST 6 / RST 7 (spec SS-2)
└── transcripts/   — NN_name.txt, replayed by tests/oracle_replay.rs and run_sim.sh
```

Capture: `scripts/build_original.sh` (the listing from `original/fetch.sh`
plus `micral_io.asm`, assembled by `platform/scripts/as8008.py`), then
`scripts/capture_oracle.py`; each transcript's header records the SHA-256 of
`build/original.bin`.

Format: `<` keys typed, `>` bytes printed (`\r \n \ \xNN` escapes), split
after each LF; `#` comments. The game halts after the final answer.
