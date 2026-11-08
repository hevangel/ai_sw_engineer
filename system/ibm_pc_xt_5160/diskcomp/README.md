# diskcomp (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `DISKCOMP.COM` of IBM PC DOS 2.00 (2,074 bytes, directory date 1983-03-08), "The IBM Personal Computer Diskette Compare Utility Version 2.0" |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT); the file's copyright line says 1982 |
| Original authors | David Litton and Mark Tyson, for IBM (named in the file); no source is published |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#291](https://github.com/hevangel/ai_sw_engineer/issues/291) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) and the host replay are identical on all 8 cases ([report](report/final_report.md)) |

## What it does

DISKCOMP compares two diskettes track by track with the BIOS (INT 13h) and reports each track that differs as
`Compare error(s) on` / `Track NN, Side N`, or `Diskettes compare ok`. It reads as many cylinders of one diskette
as fit in memory and then the other's a cylinder at a time, comparing each with `REPZ CMPSB`; the next pass swaps
the roles, so that with one drive each pass needs one swap. It finds the sides from head 1 of cylinder 0 and the
sectors per track from sector 9 (or `/1`, `/8`), and asks `Compare more diskettes (Y/N)?`. It is the companion of
[DISKCOPY](../diskcopy/README.md), by the same authors and built the same way. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, what it shares with DISKCOPY, [I/O contract and memory map](spec/io.md), deliberate differences D1-D4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over a `Pc` trait; `main.rs` the `.COM` target; `start.s` the start with the stack below the buffers (spec D1); `bios.s` INT 13h, INT 11h and the compare |
| `tests/` | `oracle_replay.rs` (host, a model of the oracle machine's BIOS, DOS and memory), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` (cases, disks, the screen rule) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule, where the rewrite lands on the disks, the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/diskcomp/scripts/run_all.sh
```

`cargo test` alone runs the tests from the spec; with `XT_ROMS` it also replays the 8 cases, and with
`build/diskcomp.com` (`scripts/build.sh`) checks that the rewrite on the disks changes no reported track.
