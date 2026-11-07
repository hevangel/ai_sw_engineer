# more (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `MORE.COM` of IBM PC DOS 2.00 (384 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT) |
| Original authors | Microsoft for IBM; Microsoft's MS-DOS 2.0 source `MORE.ASM` names no author ([reference](reference/ms-dos-2.0/README.md)) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#286](https://github.com/hevangel/ai_sw_engineer/issues/286) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) is identical on 10 cases ([report](report/final_report.md)) |

## Why this title

The third DOS 2.00 filter of the XT catalog ([../software_catalog.md](../software_catalog.md)), after SORT and FIND: it adds what they do not use, handle duplication (function 45h) to read the keyboard while standard input is redirected, a key wait (0Ch/01h) driven by the screen, and a BIOS call (INT 10h AH=0Fh) for the screen width.

## History

DOS 2.0 brought Unix-style redirection and pipes to the PC, and with them three filters: SORT, FIND and MORE. MORE shows standard input a screenful at a time (`more < file`, `type file | more`): after 24 rows it prints `-- More --` and waits for a key. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, its differences from the source (S1-S2), [I/O contract](spec/io.md), deliberate differences D1-D4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's MIT-licensed MS-DOS 2.0 `MORE.ASM` (reference only) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h target (`.COM`); `video.s` its INT 10h call |
| `tests/` | `oracle_replay.rs` (host, with a model of DOS's handles and the CGA console), `rust_emulator.rs` (original and rewrite on the Rust emulator) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh` (capture: `../scripts/capture_dos.sh`) |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule and the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/more/scripts/run_all.sh
```
