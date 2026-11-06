# find (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `FIND.EXE` of IBM PC DOS 2.00 (5,888 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT); the program's revision 6 is dated 11/10/82 in Microsoft's source |
| Original authors | Microsoft for IBM; Microsoft's MS-DOS 2.0 source of FIND names M. A. Ulloa as its writer ([reference](reference/ms-dos-2.0/README.md); Rev. 8, two revisions later) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#283](https://github.com/hevangel/ai_sw_engineer/issues/283) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) is identical on 22 cases ([report](report/final_report.md)) |

## Why this title

The second DOS 2.00 filter of the XT catalog ([../software_catalog.md](../software_catalog.md)), after SORT: it adds what SORT does not use, file handles opened by name (functions 3Dh, 3Eh, 42h), several input files, and the console's ^Z handling.

## History

DOS 2.0 brought Unix-style redirection and pipes to the PC, and with them three filters: SORT, FIND and MORE. FIND prints the lines of its files (or of standard input) that contain a quoted string; `/V` prints the others, `/C` only counts them, `/N` numbers them. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract](spec/io.md), deliberate differences D1-D4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's MIT-licensed MS-DOS 2.0 `FIND.ASM`, Rev. 8 (reference only; the binary is Rev. 6) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h target |
| `tests/` | `oracle_replay.rs` (host), `rust_emulator.rs` (original and rewrite on the Rust emulator) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh` (capture: `../scripts/capture_dos.sh`) |
| [plans/testplan.md](plans/testplan.md) | the cases and levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/find/scripts/run_all.sh
```
