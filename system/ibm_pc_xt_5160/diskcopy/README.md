# diskcopy (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `DISKCOPY.COM` of IBM PC DOS 2.00 (2,444 bytes, directory date 1983-03-08), "The IBM Personal Computer Diskette Copier Version 2.0" |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT); the file's copyright line says 1982 |
| Original authors | David Litton and Mark Tyson, for IBM (named in the file); no source is published (Microsoft's MS-DOS 2.0 `DISKCOPY.ASM` is a different program) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#289](https://github.com/hevangel/ai_sw_engineer/issues/289) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) and the host replay are identical on all 7 cases, screen and drive B ([report](report/final_report.md)) |

## What it does

DISKCOPY copies a whole diskette track by track with the BIOS (INT 13h): it reads as many tracks of the source as
fit in memory (both heads of each cylinder), asks for the target when there is one drive, writes and verifies them,
formats the target's cylinders if a write check fails, and repeats until cylinder 39. It finds the source's sides
from head 1 of cylinder 0 and its sectors per track from sector 9, reports unreadable or unwritable tracks and goes
on, and asks `Copy another (Y/N)?`. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract and memory map](spec/io.md), the head-1 difference between the emulators, deliberate differences D1-D4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160, with drive B's hash after each run |
| `src/` | `lib.rs` the `no_std` core over a `Pc` trait; `main.rs` the `.COM` target; `start.s` the start with the stack below the track buffer (spec D1); `bios.s` INT 13h and INT 11h |
| `tests/` | `oracle_replay.rs` (host, a model of the oracle machine's BIOS and DOS), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` (cases, disks, the screen and drive B rules) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rules and the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/diskcopy/scripts/run_all.sh
```

`cargo test` alone runs the host replay (with `XT_ROMS` it also compares drive B; without it drive A is a blank disk
and only the screens are compared).
