# format (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `FORMAT.COM` of IBM PC DOS 2.00 (6,016 bytes, directory date 1983-03-08) |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT) |
| Original authors | Microsoft for IBM (main module) and IBM (the OEM module that formats the diskettes, unpublished); Microsoft's generic source (MS-DOS 2.0 FORMAT.ASM "Vers 2.10", MIT) names no author; its revision notes credit ARR (the volume label prompt, 5/12/82; the 2.10 redo, 5/1/83) ([reference](reference/ms-dos-2.0/README.md)) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#288](https://github.com/hevangel/ai_sw_engineer/issues/288) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) agrees on all 12 cases, screen and drive B image, with two documented masks (the label's time stamp; the /8 boot record, spec D1) ([report](report/final_report.md)) |

## What it does

FORMAT formats the diskette in the drive given (INT 13h: every track formatted and verified, 9 sectors of F6h),
writes the PC DOS 2.00 boot record with the BPB of the layout (360 KiB, or 180 KiB with `/1`, 320 KiB with `/8`,
160 KiB with both), clears both FATs and the root directory, optionally copies IBMBIO.COM, IBMDOS.COM and
COMMAND.COM from the default drive (`/S`) and asks for a volume label (`/V`), reports the disk space and offers to
format another. Details in [spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary (Microsoft's generic part and IBM's OEM module), [I/O contract](spec/io.md), deliberate differences D1-D7 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 with drive B's SHA-256 after each run; the masked hashes |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's FORMAT.ASM, FORMES.ASM, GENFOR.ASM (MIT) and what the IBM binary shares with them |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the INT 21h `.COM` target; `bios.s` INT 25h/26h, INT 11h, INT 13h |
| `tests/` | `oracle_replay.rs` (host, a DOS 2.00 and BIOS model over the disk images), `rust_emulator.rs` (original and rewrite on the Rust emulator, drive B read back), `common/` (cases, drive B disk, comparison rules) |
| `scripts/` | `cases.py`, `build.sh`, `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` (`SAVE_B=1`) |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rules (screen and drive B, the masks) and the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/format/scripts/run_all.sh
```

`cargo test` alone runs the host replay; with `XT_ROMS` (drive A is PC DOS 2.00 disk 1) it also checks drive B's
images and the /S cases, without it only the other cases' screens. The rewrite reads the boot record and the FAT
buffer's stale bytes from drive A at run time (spec D1, D2): neither is in this source, and no disk FORMAT wrote is
committed.
