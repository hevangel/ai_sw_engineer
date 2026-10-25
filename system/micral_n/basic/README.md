# SCELBAL BASIC (micral_n)

| | |
|---|---|
| Original year | 1974 per the listing's header; © 1975 Scelbi Computer Consulting (spec "Sources") |
| Original authors | Mark G. Arnold and Nat Wadsworth |
| Machine | SCELBI 8H/8B (8008); here the Micral N |
| Status | Rust rewrite matches the original byte for byte: host 10/10, RTL 10/10 loaded directly and booted from disk. Image 13826 B of the 14080 B load area; program buffer 2560 B (spec BAS-4) |

## History

SCELBAL ("SCientific ELementary BAsic Language") was written for the SCELBI
8008 computers and published by Scelbi Computer Consulting as a book with
its full listing, *SCELBAL: A Higher-Level Language for 8008/8080 Systems*.
It is a floating point BASIC in about 7 KB of 8008 code, with its own
four-byte floating point package. The listing used here is Steve Loboyko's
2001 transcription, assembled by Thomas E. Jones's AS8 and corrected by Mark
Arnold in 2012; Arnold permits educational, historical and non-commercial
use (`original/provenance.md`). The Micral N did not run SCELBAL as far as
we know; this port gives it the Micral N teletype, as the listing's I/O
section expects its user to do.

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behavior spec, assumption ledger, [I/O contract](spec/io.md) |
| [original/](original/provenance.md) | provenance, SHA-256 and `fetch.sh` for the listing (gitignored) |
| [oracle/](oracle/README.md) | the Micral N I/O patch and the transcripts captured from the original |
| `src/` | `lib.rs` interpreter, `fp.rs` floating point, `mem.rs` the original's memory layout, `hal.rs` I/O trait, `main.rs` target/host entry |
| `asm/` | `start.s` entry and error restart, `fp.s` carry primitives for the target |
| `layout.ld` | linker layout: loaded at 0x0000 by the boot ROM |
| `tests/` | oracle replay on the host |
| [plans/](plans/implementation_plan.md) | implementation and [test plan](plans/testplan.md) |
| `scripts/` | build, oracle build and capture, host run, RTL replay, run_all |
| `build/` | generated (gitignored) |
| [report/](report/final_report.md) | final report and [size history](report/size_history.md) |

## Run

```bash
./scripts/run_all.sh                    # lint, host replay, image, RTL replay
./scripts/run_host.sh                   # SCELBAL on stdin/stdout (type SCR first)
./scripts/build_original.py             # the oracle image (fetches the listing)
./scripts/capture_oracle.py             # re-capture the transcripts from it
```

`build.sh` runs in the ai-sw-engineer image and `run_sim.sh` / `compare.sh`
in the ai-hw-engineer image (the RTL harness, `platform/sim/build_rtl.sh`).
