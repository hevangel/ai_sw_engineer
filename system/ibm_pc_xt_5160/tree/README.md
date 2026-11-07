# tree (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `TREE.COM` of IBM PC DOS 2.00 (1,513 bytes), "Directory Path Display Utility Version 1.00" |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT; the binary says `(C)Copyright IBM Corp 1983`) |
| Original authors | IBM; the binary names David Litton as its author. No source is known to be published |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#285](https://github.com/hevangel/ai_sw_engineer/issues/285) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) is identical on 9 cases ([report](report/final_report.md)) |

## Why this title

DOS 2.00 brought subdirectories to the PC, and TREE shows them: it walks a disk's directories depth first and lists each one's subdirectories and, with `/F`, its files. It exercises what the XT profile adds over the 5150 for a `.COM`: hierarchical directories (CHDIR, current directory), extended-FCB searches with attributes, and a second drive. Its output is a pure function of the disk, so the oracle is exact.

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract](spec/io.md), deliberate differences D1-D4 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h `.COM` target |
| `tests/` | `oracle_replay.rs` (host, a DOS model over the pcdisk images), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` (cases, drive B disk, comparison rule) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule and the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/tree/scripts/run_all.sh
```

`cargo test` alone runs the host replay (drive A is a blank disk unless `XT_ROMS` is set).
