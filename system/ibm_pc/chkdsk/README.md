# chkdsk (ibm_pc)

| | |
|---|---|
| Original | `CHKDSK.COM` of IBM PC DOS 1.00 (1395 bytes) |
| Original year | 1981 (PC DOS 1.00 shipped with the IBM PC, August 1981) |
| Original authors | Microsoft for IBM; the DOS 1.x CHKDSK is generally credited to Microsoft (Tim Paterson's QDOS/86-DOS lineage). Authorship of this particular program is not recorded in the binary. |
| Machine | ibm_pc — i8086 |
| Status | reimplemented in Rust; identical to the original on 24 test disks (screen and disk afterwards), on the Rust emulator and on the RTL |

## Why this is the first title

The platform so far boots bare-metal floppy images. A real title has to run on top of the thing every PC program ran on: DOS. CHKDSK is the best first one:

- **Small and closed.** 1395 bytes, no dependency on anything but INT 21h; a complete program with arguments, error paths, output formatting and 32-bit arithmetic, yet short enough to specify and test exhaustively.
- **Real, checkable behaviour.** Its output is a pure function of the disk image it is given and the memory size, so the oracle (the original run on the RTL, and on the Rust emulator) produces exact bytes to compare.
- **Exercises the new backend where it matters.** 32-bit multiply and decimal conversion (`__mulsi3`, divide), 12-bit FAT packing, byte/word mixing, `rep` string operations, tables of records: the codegen features built in `llvm-i8086`.
- **Both ends of the stack.** It forces the COM-under-DOS platform variant (link at 100h, PSP, INT 21h HAL) that every later DOS title needs, and the FAT12 disk tool [`pcdisk`](../platform/tools/pcdisk/) that builds the test disks.

Alternatives considered: `COMP`, `SYS`, `EDLIN`, `DEBUG` (bigger, interactive or tied to the BIOS/DOS internals), BASIC samples (need the ROM BASIC), bare-boot games (no host/DOS-independent oracle on this machine). The MIT-licensed MS-DOS 1.25 `CHKDSK.COM` was rejected as the oracle because it is not the program on the reference disk ([provenance](original/provenance.md)).

## History

CHKDSK checked the consistency of a FAT disk: file allocation chains, directory entries, lost clusters. PC DOS 1.00's version is the original of the tool that stayed in DOS for decades; this one reads the FAT through DOS function 1Bh, walks every file's chain, repairs what it finds and prints the disk and memory totals. Details in [spec](spec/spec.md); sources of the claims in [provenance](original/provenance.md).

## Layout

| Path | Contents |
|---|---|
| [spec/](spec/spec.md) | behavior spec and I/O contract |
| [original/](original/provenance.md) | provenance + SHA-256 of the original (binary gitignored) |
| [oracle/](oracle/README.md) | transcripts captured from the original |
| `src/` | `lib.rs` portable core, `hal.rs` I/O trait, `main.rs` target/host entry |
| `tests/` | oracle replay |
| [plans/](plans/implementation_plan.md) | implementation and [test plan](plans/testplan.md) |
| `scripts/` | build, host run, simulator run, compare, run_all |
| `bin/` | signed-off target images + `.sha256` |
| `build/` | generated IR/asm/objects/images (gitignored) |
| [report/](report/final_report.md) | final report |

## Run

```bash
cargo test                                   # the core against the 24 oracle transcripts, on the host
cargo test --release --test target_dos -- --ignored   # bin/chkdsk.com under PC DOS 1.00 on the Rust emulator
cargo test --test rtl_target                 # the Rust program vs the original, both recorded on the RTL
scripts/build_com.sh                         # rebuild bin/chkdsk.com (ai-sw-engineer image)
scripts/rtl_oracle_all.sh                    # re-record the original on the RTL (about 3 minutes per case)
scripts/rtl_target.sh                        # record the Rust program on the RTL
```

The ignored test needs the original BIOS, Cassette BASIC and PC DOS 1.00 disk (see [provenance](original/provenance.md); looked up in `$IBM_PC_ROMS`, else `$AI_HW_ENGINEER/system/ibm_pc/work/reference`).

## Result

- `bin/chkdsk.com`: 3501 bytes (the original: 1395). Rust with `core`'s code the program uses (no `core::fmt`), the DOS runtime of the platform (`crt0_com`, `dos_call`, the critical-error handler) and the compiler's runtime library, built at `opt-level=z`.
- The portable core (`src/lib.rs`, `no_std`, no allocation) calls DOS only through the `Dos` trait; `DosCom` in `src/main.rs` makes the INT 21h calls of the original, `tests/common/hostdos.rs` models them over a disk image.
- What the Rust program does that the original does, observed: 24 cases of screen text and the disk afterwards, including repairs of sizes, cut chains, cross-links, lost clusters and entries pointing outside the disk. See [the spec](spec/spec.md) for what is and is not reproduced.
