# ibm_pc emulator test plan

| Level | What | Oracle | Command (from `system/ibm_pc/emulator/`) |
|---|---|---|---|
| Units | SHA-256, scan codes, script parsing, firmware/disk size checks, CGA graphics decoding (banks, palettes, 640x200) | FIPS 180-4 vectors; hand-worked from the Technical Reference | `cargo test` |
| CPU programs | control flow, stack, interrupts and shadows, `REP` interruption, memory map, I/O | Intel manual, worked by hand | `cargo test --test cpu_programs` |
| Devices | PIC priority/EOI, PIT modes and latch, DMA flip-flop and terminal count, FDC command phases, keyboard handshake | datasheets, worked by hand | `cargo test --test devices` |
| CLI | usage and error exits | n/a | `cargo test --test cli` |
| Silicon vectors | every single-instruction state and bus-write sequence | the physical chip (SingleStepTests/8086 `e71c68d`) | `cargo test --release --test silicon_vectors -- --ignored --nocapture` |
| RTL CPU differential | boundary-biased random states, expected values from this core | the hardware project's 8086 RTL (Verilator, in Docker) | `scripts/rtl_diff.sh [cases_per_encoding] [seed]` |
| RTL system replay | unchanged BIOS + PC DOS 1.00 + typed DOS and BASIC session: screens and drive B image | the hardware project's IBM PC RTL (`Vibm_pc`) | `cargo test --release --test replay -- --ignored --nocapture` |
| Original image | other copyrighted software, supplied by the user | recorded RTL transcripts | planned |
| Rust program | `hello_rs` and `stress_rs` built by rustc (as msp430) and our `llc`, linked by `i8086-ld`: on the bare core with a BIOS-free HAL stub, and `hello_rs` booted through the original BIOS | the host: computed values, and the same source run natively | `cargo test --test i8086_rust` (`-- --ignored` for the BIOS boot and the optimisation matrix) |
| Compiled code | `llvm-i8086` objects and runtime, linked by `i8086-ld`, called with the ABI | the same computation on the host | `cargo test --test i8086_codegen` |
| Boot image vs the RTL | the two `llvm-i8086` floppy images (assembly boot program, Rust `hello_rs`) booted on the Rust emulator | the RTL booting the same images (`oracle/boot/*.txt`): screen and final CPU state | `cargo test --release --test boot_oracle -- --ignored`; record with `scripts/rtl_boot_images.sh` |
| Rust `.COM` and tiny-model `.EXE` under DOS vs the RTL | `hello_com` (llvm-i8086 Rust, `link_com.ld`, `crt0_com`, INT 21h) as `HELLO.COM` and as `HELLOX.EXE` (`i8086-ld --exe`) run from PC DOS 1.00 on the Rust emulator | the RTL running the same DOS disks (`oracle/dos/hello_com.txt`, `hello_exe.txt`): the 25 screen rows | `cargo test --release --test i8086_dos -- --ignored`; record with `scripts/rtl_dos_programs.sh` |
| Interrupt-driven Rust program vs the RTL | `irq_rs` (timer and keyboard handlers in Rust) booted on the Rust emulator, with scan codes typed between two screens | the RTL booting the same image and receiving the same keys (`oracle/boot/irq_rs.txt`): both screens; registers not compared (tick phase) | `cargo test --release --test boot_oracle -- --ignored`; record with `scripts/rtl_boot_images.sh` |
| Boot image | a floppy packed by `i8086-ld` from an assembled two-stage program: layout, stage 2 from the linked binary, and the whole boot through the original BIOS | the program's own effect (a counter, text on the CGA screen) | `cargo test --test i8086_boot` (and `-- --ignored` with the ROMs) |
| Generated image | compiled title image through the CLI | same transcripts as the target run | planned (the backend has no linker yet) |

Hygiene: `cargo fmt -- --check` and `cargo clippy --all-targets -- -D warnings`
(use `cargo +1.99.1 ...` where no default toolchain is set).

## Silicon vectors: where they come from

The 175 MB corpus is not committed. The test reads the copy cached by the hardware
project (`$I8086_VECTORS`, else `$AI_HW_ENGINEER/design/intel_8086/work/reference/v1`,
else `../ai_hw_engineer/...`); that project's `design/intel_8086/scripts/prepare_vectors.py`
downloads and hash-checks it at the pinned commit. The selection logic mirrors that
script, so the "accepted" count (566,721) is directly comparable to the RTL's.
Undefined flags are masked with the corpus's own `flags-mask`; vectors the RTL
excludes as non-normal encodings run as "extended" and are reported separately.
Filters: `I8086_OPS=00,F6` (hex opcodes), `I8086_SHOW=n` (failures shown per opcode).

## RTL CPU differential

`examples/rtl_fuzz.rs` writes vector files in the RTL runner's format with expected
values from this core; `rtl/tb_diff.cpp` includes the RTL's own testbench (unmodified,
from `$AI_HW_ENGINEER`) with `main` renamed and reports every mismatch, where the
original runner stops at the first. `scripts/rtl_diff.sh` builds the runner in the
`ai-hw-engineer:latest` image and runs it (about 15 s for 58,000 cases including the
Verilator build). Because the expected values are this core's, an agreement is
evidence of two independent implementations agreeing, not of correctness; correctness
against the chip is the silicon-vectors level. The fuzz excludes encodings outside the
RTL's contract (`0F`, register forms of far `CALL`/`JMP`, LEA/LES/LDS) and states
that modify the instruction's own bytes.

## RTL system replay

`scripts/rtl_oracle.sh` drives the RTL's `Vibm_pc` simulator (the whole 5150 on the
RTL 8088, 8259, 8253, 8255, 8237 and 8272, running the unchanged firmware) through
the session in `scripts/rtl_oracle.py` and writes
`oracle/transcripts/dos100_session.txt`: after each typed step, the 25-row text screen,
in graphics modes the SHA-256 of the 16 KiB video memory with the CGA mode and colour
registers, and the SHA-256 of drive B. `cargo test --test replay -- --ignored` runs the same
steps on the Rust machine and compares the transcripts exactly; the same comparison
is available from the command line (`ibm_pc-emulator ... session.txt | diff`). The
firmware's SHA-256s are checked first (`original/provenance.md`). Both machines run
the same code, so timing-dependent output (clock counts, instruction counts) is not
compared, only what a user sees once the machine has settled.

Coverage goal: every peripheral and mode in `spec/spec.md` is exercised by at
least one test; every unsupported item is listed there.
