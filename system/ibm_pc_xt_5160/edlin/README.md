# edlin (ibm_pc_xt_5160)

| | |
|---|---|
| Original | `EDLIN.COM` of IBM PC DOS 2.00 (4,608 bytes, directory date 1983-03-08), "The IBM Personal Computer EDITOR Version 2.00" |
| Original year | 1983 (PC DOS 2.00, released with the IBM PC XT); the file's copyright line says 1981, 1982; Microsoft's revision 12, dated 12/29/82 in the source's revision history |
| Original authors | Microsoft for IBM: M. A. Ulloa (revisions V2.00 to 12, named in the file as `MAUlloa/Microsoft`), with A. Reynolds (the messages, revision 9); Microsoft's source of revision 14 adds N. Panners ([reference](reference/ms-dos-2.0/README.md)) |
| Machine | ibm_pc_xt_5160 — i8086 |
| Issue | [hevangel/ai_sw_engineer#293](https://github.com/hevangel/ai_sw_engineer/issues/293) |
| Status | reimplemented in Rust; the four-way matrix (original/rewrite × MartyPC 5160/Rust emulator) and the host replay are identical on all 14 cases ([report](report/final_report.md)) |

## What it does

EDLIN is DOS's line editor. It reads a text file (up to three quarters of memory at first) and edits it by line
numbers at a `*` prompt: L and P list, I inserts, D deletes, a number alone edits that line with the old one as the
template of DOS's line input, S and R search and replace (with `?` asking `O.K.?`), M and C move and copy blocks, T
merges another file, W and A write and read when the file is larger than memory, E writes the file (keeping the old
one as NAME.BAK) and Q abandons it. Several commands may share a line, separated by `;`. Details in
[spec](spec/spec.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behaviour from the binary, [I/O contract and memory map](spec/io.md), deliberate differences D1-D5 |
| [original/](original/provenance.md) | provenance and SHA-256 of the original (binary gitignored) |
| [reference/ms-dos-2.0/](reference/ms-dos-2.0/README.md) | Microsoft's MIT-licensed MS-DOS 2.0 `EDLIN.ASM`, `EDLPROC.ASM`, `EDLMES.ASM`, revision 14 (reference only; the binary is revision 12, the differences listed there) |
| [oracle/](oracle/README.md) | transcripts of the original on MartyPC's IBM 5160 |
| `src/` | `lib.rs` the `no_std` core over a `Dos` trait; `main.rs` the target; `start.s` the start of the load-high program (spec D1); `edlin.s` INT 21h with Ctrl-C caught, INT 23h/24h, INT 20h and the string instructions on the PSP's segment (spec D2) |
| `tests/` | `oracle_replay.rs` (host, a model of the oracle machine's DOS session), `rust_emulator.rs` (original and rewrite on the Rust emulator), `common/` (cases, disks, the screen rule) |
| `scripts/` | `cases.py`, `build.sh`, `mkexe.py` (the load-high MZ file), `compare.py`, `run_all.sh`; captures with `../scripts/capture_dos.sh` |
| [plans/testplan.md](plans/testplan.md) | the cases, the comparison rule, the levels |
| [report/](report/final_report.md) | results |

## Run

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR system/ibm_pc_xt_5160/edlin/scripts/run_all.sh
```

`cargo test` alone runs the tests from the spec and replays 13 cases; with `XT_ROMS` it replays all 14 (e09 needs
COMMAND.COM's bytes, which the oracle machine has in memory).
