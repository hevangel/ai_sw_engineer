# DEBUG test plan

The oracle is the original `DEBUG.COM` of PC DOS 2.00 run by PC DOS 2.00 on MartyPC's IBM 5160
(256 KiB) with the original XT BIOS, DEBUG.COM copied from disk 2 (`../scripts/capture_dos.sh` →
`oracle/transcripts/`). Each case (`scripts/cases.py`; its output is split on `\n` only, since a key
sequence ends with `\r`) is a command typed at `A>`, the files added to drive A, and (wait text,
keys) pairs for what is typed after it. A wait text starting with `$` means the last non-blank row
ends with the rest, the screen having changed since the previous wait (`until endswith`): `$-` the
`-` prompt, `$:` the `R <reg>` value prompt, `$` A's address prompt (any change), `$A>` COMMAND's
prompt. Every case ends with Q; a case that writes a file then TYPEs it, so the screen shows what
DEBUG wrote. Every level builds the drive-A disk the same way (`tests/common/mod.rs` `disk_a`).

## What is compared

**The screen**, empty rows included, right-trimmed: if the oracle shows the case's command line
(`A>` + the command), the rows from the one after it to the next row that is only `A>`, in the
oracle and in the run; otherwise the oracle's rows from the top through its final `A>` against the
same number of the run's rows ending with its final `A>`. The rule is in `tests/common/mod.rs`
(`compared`) and `scripts/compare.py`. On the Rust emulator the files DEBUG leaves on drive A are
compared too: the Rust build's must be the original's, byte for byte.

| Case | Command / keys | What it checks (spec) |
|---|---|---|
| g01 | `debug`: `r`, `q` | the register display, flags, and the CS:IP disassembly with the `DS:0000=CD` annotation (DISPREG) |
| g02 | `e 200 ...`, `d 200 l 20`, `q` | E (list form with a string), D (two full rows) |
| g03 | `f 300 l 20 ...`, `s 300 l 40 ...` ×2, `q` | F fill, S (16 matches, then none) |
| g04 | `f`, `m`, `e 505 9`, `c`, `q` | M move, C compare (one mismatch line) |
| g05 | `h 1234 abcd`, `h ffff 1`, `q` | H (sum and difference, wrapping) |
| g06 | `a 100` + five instructions, `u 100 l 10`, `q` | the assembler and the disassembler agree on the bytes |
| g07 | `a 100` + four instructions, `t`, `t`, `g`, `q` | T single-step (registers, flags, next line), G to `Program terminated normally` |
| g08 | `r ax`:`1234`, `r f`:`ov dn`, `r`, `q` | set a register (`:` prompt), edit the flags, the result shown |
| g09 | `debug fruit.txt`: `d 100 l 20`, `r`, `q` | a binary loaded on the command line (CX = size), D, the `DB` of a data byte |
| g10 | `n out.dat`, `e 100 "hello" 0d 0a`, `r cx`:`7`, `w`, then `type out.dat` | N, W (`Writing 0007 bytes`), the file written |
| g11 | `zz`, `d 100 l`, `e`, `q` | `^ Error` at the right column for three bad commands |
| g12 | `debug sort.exe`: `r`, `u l 6`, `q` | an `.EXE` EXEC-loaded (the original's segments 0905/0915/0939), `u l 6` as an error |

| Level | Runs | Expected | Command |
|---|---|---|---|
| Host | the Rust core with a model of the oracle machine's DOS session (flat memory, the console, a file map, a tiny interpreter for the T/G user code, the EXE read through the file calls; SORT.EXE from disk 2 needs XT_ROMS, else g12 is not compared) | the oracle rows | `cargo test` |
| Original × Rust emulator | the original under PC DOS 2.00 on `system/ibm_pc/emulator` (`Machine::new(256)`) | the oracle rows | `cargo test --release --test rust_emulator -- --ignored` |
| Rewrite × Rust emulator | `build/debug.com` as DEBUG.COM there | the oracle rows; the same screen and files as the original | same |
| Rewrite × MartyPC 5160 | `build/debug.com` as DEBUG.COM on the same disks | the oracle rows | `capture_dos.sh` with `PROGRAM=`, then `compare.py` |
| Original × MartyPC 5160 | the oracle itself | — | `capture_dos.sh` (never without `PROGRAM=` except to re-capture the oracle) |

The host's T/G interpreter runs only the few instructions the cases execute (MOV imm/reg, INC, INT);
the emulator and MartyPC cells run T and G on a real 8086 under real DOS (spec/io.md). No case
disassembles an 8087 instruction (spec D-DIS) or assembles a form outside D-ASM.
