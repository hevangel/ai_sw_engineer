# print (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `PRINT.COM` of IBM PC DOS 2.00 (4,608 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT; the file's directory date is 1983-03-08; the binary carries no copyright or version text) |
| Original authors | IBM (or Microsoft for IBM); the binary names no author and no source is known to be published |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#297](https://github.com/hevangel/ai_sw_engineer/issues/297) |
| Status | reimplemented (Rust transient part, assembly resident part); the four-way matrix (original/rewrite × 86Box XT with a text printer/Rust emulator with its printer) and the host replay are identical on all 10 cases, screen and printed lines ([report](report/final_report.md)) |

## What it does

`print a.txt b.txt` queues files for the background printer. The first run asks `Name of list device [PRN]:`,
opens that device, hooks INT 1Ch (timer), 28h (DOS idle), 2Fh (its requests), 13h, 17h or 14h, and 5, and stays
resident in 0BFAh bytes (INT 27h). Every ninth timer tick outside DOS, and at every DOS idle call, the resident
part reads the head file in 512-byte records with FCB calls and sends it, one byte at a time, straight to the
device's driver: tabs as spaces, up to the first ^Z, then a form feed. `/C` cancels and `/P` queues the names
around it, `/T` cancels everything; each run then lists the queue. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract and resident layout](spec/io.md), deliberate differences D1-D3, the host model's assumptions H1-H4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on 86Box's IBM PC/XT: screen rows and printed lines |
| `src/` | `lib.rs` the `no_std` core: the transient part (`run`) over a `Dos` trait and the resident part's logic (`resident`, for the host); `resident.s` the resident part at the original's offsets, the start at BFAh and the INT 2Fh/27h helpers (spec D1, D2); `main.rs` the target's `Dos` |
| `tests/` | `oracle_replay.rs` (host: the core and the resident model in front of a model of DOS and COMMAND over the pcdisk image, the printer renderer's test, the spec's tests), `rust_emulator.rs` (original and rewrite on the Rust emulator with its printer; the build's layout), `common/` (cases, disk, the screen rule, the rendering of printed bytes as 86Box's text printer writes them, the program image) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` (`ORACLE=86box`) |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rules, the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR XT86_ROMS=DIR BOX86=FILE MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR \
  system/ibm_pc_xt_5160/print/scripts/run_all.sh
```

`cargo test` alone runs the host replay of the 10 cases (drive A a blank disk unless `XT_ROMS` is set) and the
tests from the spec. The rewrite on 86Box alone:

```bash
ORACLE=86box BOX86=FILE XT86_ROMS=DIR XT_ROMS=DIR PROGRAM=$PWD/build/print.com NAME=PRINT.COM \
  OUT=$PWD/build/rust_on_86box WORK=$PWD/build/rust_run bash ../scripts/capture_dos.sh $PWD
python3 -I scripts/compare.py oracle/transcripts build/rust_on_86box
```

Never run `capture_dos.sh` without `PROGRAM` unless the oracle is to be re-captured.
