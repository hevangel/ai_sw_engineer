# MORE (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#286](https://github.com/hevangel/ai_sw_engineer/issues/286). Run of 2026-11-07 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `MORE.COM` from PC DOS 2.00 disk 1, 384 bytes, SHA-256 `fa8acd69...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS: 10 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, 64 KiB, PC DOS 2.00 from a 180 KiB disk): **identical to MartyPC on all 10** |
| 3. Source | Microsoft's MS-DOS 2.0 `MORE.ASM` (MIT), the `IBMVER` build of this program except for two instructions (`dec si; inc cx` at 1DCh, in the binary only: the byte that filled the screen is shown again) ([reference](../reference/ms-dos-2.0/README.md)) |
| 4. Documents | no IBM manual page fetched; the binary and the source are the documents used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 384-byte image read in full with GNU objdump 2.42 (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std` core over a `Dos` trait, one method per call the original makes), `src/main.rs` (INT 21h 30h, 09h, 45h, 3Eh, 3Fh, 02h, 0Ch/01h, INT 20h, `.COM`), `src/video.s` (INT 10h AH=0Fh) |
| 7-8. Four-way matrix | identical on all 10 cases, below |

## The matrix

What is compared: the rows from the one after the command line to the next `A>` prompt, empty rows included (m01, m05, m06), or, where the command line has scrolled off (the other seven), the oracle's screen from the top through its last `A>` against the tail of the run ([test plan](../plans/testplan.md)).

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00) |
|---|---|---|
| original MORE.COM | the oracle (10 transcripts) | 10 of 10 identical (`tests/rust_emulator.rs`) |
| Rust MORE.COM | 10 of 10 identical (`capture_dos.sh` with `PROGRAM=`/`NAME=MORE.COM`, `compare.py`) | 10 of 10 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: the core with a model of DOS's handles and the CGA console, each key given only when MORE waits and its wait text holds on the drawn screen) is also 10 of 10, plus three rule tests (the version check; MAXCOL from INT 10h in a 40-column mode; rows counted across 4,096-byte reads). Every level agreed on the first run.

Found while getting there: the published source would show line 25 right after `-- More --`; the binary's extra `dec si; inc cx` (spec S1) is why the oracle has an empty row after every prompt and shows the 80th character of a wide line twice (m04). Microsoft's own `MSVER` build has the same two instructions.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR system/ibm_pc_xt_5160/more/scripts/run_all.sh
```

## Size

`build/more.com`: 822 bytes (the original: 384, of which 102 are the zero start of its buffer), SHA-256 `1c070c5aa5888c6eeafb995cd4170cc4b35473afff40845897440e98d8541624`, with the shared DOS runtime: `.text` 777 bytes (the core in `main` 498, `dos_call` 83, `crt0_com` 45, `more_video_columns` 13), `.rodata` 45 (the three messages), `.bss` 4,098 (the 4,096-byte buffer and the runtime's word): 4,920 of 65,024 bytes of the segment. Built by the msp430 Rust route (nightly-2026-10-15) and `llvm-i8086`.

## Limits and deliberate differences

- D1-D4 in the spec change nothing observable: the buffer as a static array of the same 4,096 bytes, the four counters as locals with the original's values after its start, INT 10h AH=0Fh from a 13-byte assembly function (the shared `ibm_pc_dos` crate has INT 21h only), the stack where the shared runtime puts it.
- Kept as the original has them: an error from function 3Fh taken as a byte count (carry not checked), the 8-bit wrapping counters, a TAB never compared with MAXCOL.
- Not run on the hardware project's XT SV machine or on hardware.
- The Rust emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29).
- Not covered on screen: DOS 1.x and a 40-column mode (host tests only), read errors, output redirected, ^C at the prompt.
