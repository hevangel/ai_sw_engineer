# comp (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `COMP.COM` of IBM PC DOS 2.00 (2,523 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT) |
| Original authors | IBM; the program's own (unprinted) banner names David Litton as its author ([provenance](original/provenance.md)); no source published |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#284](https://github.com/hevangel/ai_sw_engineer/issues/284) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) is identical on 10 cases ([report](report/final_report.md)) |

## What it does

COMP compares two files, or every file matching a pattern with its counterpart (`comp *.dat *.bak`, or a drive or directory as the second name). It prints the offset and both bytes of each difference, stops after ten, says whether the sizes differ or the first file lacks a ^Z end mark, and asks whether to compare more. It works with FCB calls, DOS 1 style, under a DOS 2.00 that it requires exactly. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract](spec/io.md), deliberate differences D1-D5 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| `oracle/transcripts/` | the original on MartyPC's IBM 5160 (`../scripts/capture_dos.sh`) |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h target (`.COM`) |
| `tests/` | `oracle_replay.rs` (host, with a model of DOS's FCB calls), `rust_emulator.rs` (original and rewrite on the Rust emulator) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule and the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/comp/scripts/run_all.sh
```
