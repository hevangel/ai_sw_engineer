# chkdsk (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `CHKDSK.COM` of IBM PC DOS 2.00 (6,400 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT) |
| Original authors | Microsoft for IBM; Microsoft's source of the program (MS-DOS 2.0, "Version 2.30", MIT) names no author; its revision notes credit NANCYP (extents) and AARONR (volume ID) ([reference](reference/ms-dos-2.0/README.md)) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#287](https://github.com/hevangel/ai_sw_engineer/issues/287) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) is identical on 15 cases ([report](report/final_report.md)) |

## What it does

CHKDSK checks a disk's FAT and directory tree: it walks every directory (extended-FCB searches, CHDIR into each),
follows every file's cluster chain, and reports cross-linked files, chains with invalid clusters, sizes that do not
fit the allocation, damaged `.`/`..` entries and lost chains, which it can turn into `FILEnnnn.CHK` files. With `/F`
it writes the fixes with INT 26h; with `/V` it lists every directory and file; with a file name it reports
fragmented files. It ends with the disk and memory report. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary and Microsoft's source, [I/O contract](spec/io.md), deliberate differences D1-D7 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's CHKDSK.ASM, CHKPROC.ASM, CHKMES.ASM (MIT) and how the IBM binary differs |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h `.COM` target; `disk.s` INT 25h/26h |
| `tests/` | `oracle_replay.rs` (host, a DOS 2.00 model over the disk images), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` (cases, drive B disk, comparison rule) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule and the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/chkdsk/scripts/run_all.sh
```

`cargo test` alone runs the host replay (drive A is PC DOS 2.00 disk 1 with `XT_ROMS`, else a blank disk; no case checks drive A).
