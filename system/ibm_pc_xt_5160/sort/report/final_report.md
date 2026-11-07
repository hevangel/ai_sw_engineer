# SORT (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#281](https://github.com/hevangel/ai_sw_engineer/issues/281). Run of 2026-11-06 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `SORT.EXE` from PC DOS 2.00 disk 1, SHA-256 `755a3c1c...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS: 16 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, 64 KiB, PC DOS 2.00 from a 180 KiB disk): **identical to MartyPC on all 16** |
| 3. Source | Microsoft's MS-DOS 2.0 `SORT.ASM` (MIT), a later version: collating table and three fixes dated after this binary ([reference](../reference/ms-dos-2.0/README.md)) |
| 4. Documents | no IBM manual page fetched; the binary and the later source are the documents used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 768-byte load module read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait, the original's buffer layout and algorithm), `src/main.rs` (INT 21h) |
| 7-8. Four-way matrix | identical on all 16 cases, below |

## The matrix

What is compared: every screen row from the command line to the next `A>` prompt, empty rows included ([test plan](../plans/testplan.md)).

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00) |
|---|---|---|
| original SORT.EXE | the oracle (16 transcripts) | 16 of 16 identical (`tests/rust_emulator.rs`) |
| Rust SORT.EXE | 16 of 16 identical (`capture_oracle.sh` with `PROGRAM=`, `compare.py`) | 16 of 16 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: the core with a DOS model, its output drawn as the console draws it) is also 16 of 16, plus a test of the switch parser.

Found while getting there: the first core rewrote each length word with CR LF before stepping by it (the original keeps the length in CX); the host replay showed the run-together lines and it was fixed.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/sort/scripts/run_all.sh
```

## Size

`build/sort.exe`: 2,411 bytes (the original: 1,280), SHA-256 `52dfa7463733f87952e1ea1224e030d375d021d6dc0641bfb139deda36dd8a52`, a tiny-model `.EXE` with the shared DOS runtime; code and data 1,899 bytes in the segment plus the 32,767-byte buffer as `.bss` (34,668 of 65,024 bytes of the segment). Built by the msp430 Rust route and `llvm-i8086` (LLVM 23.1.3 `0d261d1c`).

## Supporting changes

- `system/ibm_pc/emulator`: the diskette controller takes 160, 180, 320 and 360 KiB images (per-disk geometry; 160 KiB unchanged; the RTL session replay still identical; 6 new device tests); `<`, `>`, `|`, backslash and backquote have keys in typed sessions. Emulator suite: 87 passed, 2 failing only for inputs not in this container (the stress objects and the silicon-vector corpus).
- `tools/martypc_oracle`: shifted characters (`< > + |`, capitals) can be typed; `MARTY_WRITABLE=1` mounts drive A writable.

## Limits and deliberate differences

- D1: the buffer is 32,767 bytes (the largest Rust object with 16-bit pointers) in the program's own segment, so `Insufficient memory` comes at about 32 KiB of input instead of about 64 KiB. D2-D4 in the spec change nothing observable.
- Not run on the hardware project's XT SV machine (ai_engineer#83; not reachable here) or on hardware.
- The Rust emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29).
- Not covered by a case: inputs over the buffer, a write error, DOS 1.x, the exit code.
