# Shooting Stars (micral_n)

| | |
|---|---|
| Original year | 1976 (*BYTE*, May 1976, cover story) |
| Original authors | Willard I. Nico (Delta t, Houston) |
| Machine | SCELBI 8H (8008); MO5 ported it to the Micral N in 2022 (spec) |
| Status | Rust rewrite matches the original byte for byte: host 4/4, RTL 4/4 loaded directly and booted from disk. Image 2458 B |

## History

A 3×3 star-field puzzle published with its 8008 listing in *BYTE*, May
1976. MO5 ported it to their restored Micral N in January 2022 and ran it
on the machine in March 2022 (HxC restoration timeline). The listing used
here is Jim Loos's transcription for the AS assembler
(`original/provenance.md`).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | game and terminal behaviour, assumption ledger SS-1..2 |
| [original/](original/provenance.md) | provenance, SHA-256 and `fetch.sh` (gitignored listing) |
| [oracle/](oracle/README.md) | the Micral N I/O patch and the transcripts captured from the original |
| `src/` | `lib.rs` game, `hal.rs` I/O trait, `main.rs` target/host entry |
| `asm/start.s` | entry at 0 and `__leaf_halt` |
| `layout.ld` | loaded at 0x0000 by the boot ROM |
| `tests/` | oracle replay on the host |
| [plans/](plans/testplan.md) | test plan |
| `scripts/` | build, oracle build and capture, host run, RTL replay, run_all |
| [report/](report/final_report.md) | final report |

## Run

```bash
./scripts/run_all.sh            # lint, host replay, image (ai-sw-engineer); RTL replay (ai-hw-engineer)
./scripts/run_host.sh           # play on stdin/stdout
./scripts/build_original.sh     # the oracle image (fetches the listing)
./scripts/capture_oracle.py     # re-capture the transcripts
```
