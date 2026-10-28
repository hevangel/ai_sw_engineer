# chkdsk oracle

Expected behavior captured from the **original** program running on a simulator of the real machine. Never derived from this repository's Rust.

```text
oracle/
├── transcripts/   — *.txt input/output transcripts replayed by tests/oracle_replay.rs
├── traces/        — optional instruction/bus traces (compressed if large)
└── capture.md     — how each file was captured: simulator, revision, original SHA-256, command
```

Transcript format: `> hex bytes` for inputs, `< hex bytes` for outputs, `#` comments. Name files `NNN_description.txt`.
