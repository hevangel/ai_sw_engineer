# SCELBAL I/O contract

SCELBAL talks to a teletype and can halt. `src/hal.rs` is the contract.

| Hal method | Micral N device | Encoding | Notes |
|---|---|---|---|
| `getc` | teletype input (`INP 0`, platform io_map.md) | 7-bit ASCII | `platform/runtime/tty.s` `__leaf_tty_getc`; SCELBAL sets bit 7 and echoes the key (spec BAS-1) |
| `putc` | teletype output (`OUT 8`) | 7-bit ASCII | `__leaf_tty_putc`; SCELBAL's own codes carry bit 7, cleared here |
| `halt` | `HLT` | — | UDF and the listing's safety halts (`asm/start.s` `__leaf_halt`) |

Transcripts (`oracle/transcripts/*.txt`): `<` lines are the keys typed, `>`
lines the bytes printed, with `\r \n \\ \xNN` escapes, split after each LF.
The keys of a session go in as one stream; the output must match exactly,
and every key must be read. A session ends when SCELBAL waits for a key and
none is left (on the RTL, `--idle`; on the host, the mock's `getc` unwinds).
