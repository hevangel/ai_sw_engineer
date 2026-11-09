# debug (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `DEBUG.COM` of IBM PC DOS 2.00 (11,904 bytes), Microsoft's debugger built with `IBMVER` ("DEBUG-86 8086 debugger", the source's `Vers 2.30`; the IBM binary drops that header) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT); the program's copyright is IBM/Microsoft 1981-1983 |
| Original authors | Microsoft: the debugger with AaronR's direct-I/O rework, C. Peters' line-by-line assembler, and zibo's trace-over and dump formatting (named in the source revision history) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#295](https://github.com/hevangel/ai_sw_engineer/issues/295) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) and the host replay are identical on all 12 cases ([report](report/final_report.md)) |

## What it does

DEBUG is DOS's assembly-level debugger. At a `-` prompt it examines and changes memory and
registers and runs programs: **R** shows and sets registers and flags, **D** dumps memory, **E**
enters bytes, **F** fills, **S** searches, **M** moves, **C** compares, **H** adds and subtracts
hex, **A** assembles and **U** disassembles 8086 code, **T** traces (single-steps) and **G** runs
with breakpoints, **N** names and **L**/**W** load and write files (a `.EXE` or `.COM` on the
command line is loaded ready to debug), and **Q** quits. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract and memory map](spec/io.md), deliberate differences D1-D-EXEC |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's MIT-licensed MS-DOS 2.0 DEBUG source (reference only; the IBM binary differs by the dropped header) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over `Dos`; `disasm.rs`, `asm.rs`, `tables.rs`; `main.rs` the target; `start.s`, `debug.s` the load-high start and machine side (spec D1, D2) |
| `tests/` | `oracle_replay.rs` (host), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` |
| `scripts/` | `cases.py`, `build.sh`, `mkexe.py`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule, the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/debug/scripts/run_all.sh
```

`cargo test` alone runs the host replay (12 cases; g12 needs `XT_ROMS` for SORT.EXE from disk 2,
else 11 are compared).
