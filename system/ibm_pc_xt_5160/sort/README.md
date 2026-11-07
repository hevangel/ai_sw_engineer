# sort (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `SORT.EXE` of IBM PC DOS 2.00 (1,280 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT) |
| Original authors | Microsoft for IBM; Microsoft's MS-DOS 2.0 source of SORT names Chris Peters as its writer ([reference](reference/ms-dos-2.0/README.md); a later version of the program) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#281](https://github.com/hevangel/ai_sw_engineer/issues/281), child of [hevangel/ai_engineer#84](https://github.com/hevangel/ai_engineer/issues/84) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) is identical on 16 cases ([report](report/final_report.md)) |

## Why this title

The first title of the XT catalog ([../software_catalog.md](../software_catalog.md)): the smallest complete program that needs what the XT profile adds over the 5150, namely DOS 2.00 with its handle-based I/O, redirection and pipes, and the `.EXE` format. Its output is a pure function of its input and switches, so the oracle is exact, and its 768-byte load module can be specified in full.

## History

DOS 2.0 brought Unix-style redirection and pipes to the PC, and with them three filters: SORT, FIND and MORE. SORT reads standard input, sorts its lines and writes them to standard output; `/R` reverses the order, `/+n` sorts on the text from column n. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract](spec/io.md), deliberate differences D1-D4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's MIT-licensed MS-DOS 2.0 `SORT.ASM`, a later version (reference only) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h target |
| `tests/` | `oracle_replay.rs` (host), `rust_emulator.rs` (original and rewrite on the Rust emulator) |
| `scripts/` | `cases.py`, `capture_oracle.sh`, `build.sh`, `compare.py`, `run_all.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases and levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/sort/scripts/run_all.sh
```
