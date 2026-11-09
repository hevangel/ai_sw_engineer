# ASSIGN (PC DOS 2.00) for the IBM PC XT 5160: report

Issue [hevangel/ai_sw_engineer#296](https://github.com/hevangel/ai_sw_engineer/issues/296). Run of 2026-11-09 on a Linux host.

## Steps of the title lifecycle (spec/software-lifecycle.md)

| Step | Result |
|---|---|
| 1. Original | `ASSIGN.COM` from PC DOS 2.00 disk 1, 896 bytes, SHA-256 `080db454...` ([provenance](../original/provenance.md)); not committed |
| 2. Original on the emulators | MartyPC `05c0d088` IBM 5160 with the original XT BIOS, 256 KiB: 8 transcripts ([oracle](../oracle/README.md)); our Rust emulator (5150 board, `Machine::new(256)`, two drives, the same disks): **identical to MartyPC on all 8** |
| 3. Source | none known to be published |
| 4. Documents | no manual page fetched; the binary is the document used |
| 5. Reverse engineering | [spec](../spec/spec.md) and [I/O](../spec/io.md) from the 896 bytes read in full with GNU objdump (REA needs a Ghidra-class provider, not installed here) |
| 6. Rust | `src/lib.rs` (`no_std`: the transient part over a `Dos` trait; `hook`, the resident logic for the host), `src/resident.s` (the resident data and hooks, spec D1-D2), `src/main.rs` (the target's INT 21h/INT 11h calls) |
| 7-8. Four-way matrix | identical on all 8 cases, below |

## The matrix

What is compared ([test plan](../plans/testplan.md)): the rows from the case's first command line through the screen's last `A>`.

| | MartyPC IBM 5160 (XT BIOS, PC DOS 2.00) | Rust emulator (5150 BIOS, PC DOS 2.00, 256 KiB) |
|---|---|---|
| original ASSIGN.COM | the oracle (8 transcripts) | 8 of 8 identical (`tests/rust_emulator.rs`) |
| Rust ASSIGN.COM | 8 of 8 identical (`capture_dos.sh` with `PROGRAM=`, `compare.py`) | 8 of 8 identical (`tests/rust_emulator.rs`) |

The host replay (`tests/oracle_replay.rs`: `run` and the `hook` model in front of a model of DOS 2.00 and COMMAND over the cases' pcdisk images, spec H1-H3) is also 8 of 8, with and without `XT_ROMS`, plus 7 tests from the spec of what no case shows (the parse, a second run replacing the resident table, FCB drive 0 and extended FCBs, 0Eh/19h/47h, the signed lookup, INT 25h/26h, the equipment routine, DOS 1).

No emulator change was needed, and the Rust emulator agrees with MartyPC on the original in every case: the resident hook, function 31h, the reload of COMMAND's transient part and the second drive all behave alike.

## Commands

```bash
IBM_PC_ROMS=DIR XT_ROMS=DIR MARTY_TARGET_DIR=DIR CARGO_TARGET_DIR_LD=DIR system/ibm_pc_xt_5160/assign/scripts/run_all.sh
```

## Size

`build/assign.com`: 1,707 bytes (the original: 896), SHA-256 `4c2f64b0c8fd9b4b792cafa30ca39b3f9903c03acf76486fb1ea8abdd4cb67f9`. The resident part is the original's: PSP:0000h-030Fh (31h paragraphs), with the data at 100h-121h and the hooks at 122h-2D2h (433 bytes of code; the original's 122h-302h, 481 bytes), zeros to 302h, the start at 303h. The transient part (freed by function 31h) is 1,150 bytes of code from 303h (the start, `dos.s`, the Rust core) and 42 bytes of messages; no `.bss`. Built by the msp430 Rust route and `llvm-i8086`.

## Deliberate differences and limits

- D1: the resident hooks are assembly (`src/resident.s`), written from the specification: a hook that runs on the caller's stack and DS, takes the return address off the stack and chains with `JMP FAR`/`CALL FAR` from fixed offsets below the transient code cannot be compiled Rust here (no inline assembly; compiled code needs DS = SS = CS and a frame). The same logic is `software::hook` in Rust, which the host replay runs.
- D2: the resident data, the hook's entry (122h), the `JMP` (E9 00 02, so the table's entry 0 is 02h as in the original) and the kept size are the original's; the code bytes from 122h are the rewrite's own. Only lookups with a drive number of 20h-7Fh (a path letter of 80h or above) read code bytes and so differ; the 8-byte signature differs in its fifth byte (1Bh against 2Dh), so the rewrite and the original do not find each other's resident copy (each would install another). No case mixes them.
- D3: the transient part is Rust with its own start at 303h; the messages and exit codes are the original's.
- Found in the binary, not run on the original: after the identity table is put back the hook passes 19h through, so the prompt shows the physical drive the install selected (`B>` after `assign a=b` then `assign` run from B); the second path of function 56h, 1Ch and handle calls are not translated; a copy behind a later INT 21h hook is not found.
- The Rust-emulator half runs on the 5150 board model, not an XT (the XT's Rust emulator is ai_sw_engineer#29). Not run on hardware.
- Not covered by a case: INT 25h/26h, EXEC with a drive letter, the equipment routine on a one-drive machine, DOS 1.
