# SCELBAL oracle

Expected behaviour captured from the **original** program on the
ai_hw_engineer 8008 RTL (Micral N harness, `platform/sim`). Never derived
from this repository's Rust.

```text
oracle/
├── micral_io.asm  — Micral N teletype routines overlaid on the listing (spec BAS-1)
└── transcripts/   — NN_name.txt, replayed by tests/oracle_replay.rs and run_sim.sh
```

Capture: `scripts/build_original.py` (needs `original/sc1.lst`, fetched by
`original/fetch.sh`), then `scripts/capture_oracle.py`, which records the
SHA-256 of `build/original.bin` in every transcript's header. The scenarios
(the keys) are in `capture_oracle.py`.

Transcript format: `<` lines are keys typed, `>` lines bytes printed, with
`\r \n \\ \xNN` escapes; output lines are split after each LF; `#` lines are
comments. The keys of a session form one stream.
