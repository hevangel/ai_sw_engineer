# assign (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `ASSIGN.COM` of IBM PC DOS 2.00 (896 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT; the file's directory date is 1983-03-08; the binary carries no copyright or version text) |
| Original authors | IBM (or Microsoft for IBM); the binary names no author and no source is known to be published |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#296](https://github.com/hevangel/ai_sw_engineer/issues/296) |
| Status | reimplemented (Rust transient part, assembly resident hooks); the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) and the host replay are identical on all 8 cases ([report](report/final_report.md)) |

## What it does

`assign a=b` makes DOS calls for drive A go to drive B: ASSIGN hooks INT 21h (and INT 25h/26h), translating the
drive in FCBs, in path names and in DL, answering function 19h with the drive the user selected, and stays resident
in 784 bytes (function 31h). Run again, it finds its resident copy through the INT 21h vector and replaces the
table; `assign` alone puts the identity back. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract and resident layout](spec/io.md), deliberate differences D1-D3, the host model's assumptions H1-H3 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core: the transient part (`run`) over a `Dos` trait and the hook's logic (`hook`, for the host); `resident.s` the resident data and hooks at the original's offsets and the start at 303h (spec D1, D2); `main.rs` the target's `Dos` |
| `tests/` | `oracle_replay.rs` (host: the core and the hook model in front of a model of DOS and COMMAND over the pcdisk images), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` (cases, disks, the screen rule) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule, the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR system/ibm_pc_xt_5160/assign/scripts/run_all.sh
```

`cargo test` alone runs the host replay of the 8 cases (drive A a blank disk unless `XT_ROMS` is set) and the tests
from the spec.
