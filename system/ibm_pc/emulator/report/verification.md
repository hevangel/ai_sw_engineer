# ibm_pc emulator verification

## 2026-10-25 (sixth record): a linked boot image

The emulator now uses the `llvm-i8086` linker (`llvm-i8086/asm`, a dev-dependency) to
link the execution tests, and boots a floppy image the linker packed.

| Command | Result |
|---|---|
| `cargo test --test i8086_codegen` | 13 of 13, now linked by `i8086-ld` with section garbage collection (the test also asserts that the runtime routines an object does not use are dropped) |
| `cargo test --test i8086_boot` | 3 of 3 always: the layout of the packed disk (160 KiB, `55 AA`, `_start` at 7C00, stage 2 at 7E00, `.bss` at 0500, the unused section dropped), stage 2 run straight from the linked binary (the counter in `.bss` is 3 and the CGA screen shows the string), the ELF header |
| `cargo test --release --test i8086_boot -- --ignored` | the original BIOS (SHA-256 checked) boots the packed floppy: it reads the boot sector, the boot sector reads sector 2 with INT 13h, stage 2 runs; the counter is 3 and row 0 of the screen is `llvm-i8086: stage 2 ok` |
| negative control | a deliberately wrong expected string fails all three screen checks |
| `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` | clean |

This boots a program, built by our own toolchain, through the unmodified BIOS on the
Rust machine. It is not compared with the RTL (that run, and a Rust-compiled boot
program, belong to llvm-i8086 milestone 7).

## 2026-10-25 (fifth record): the `llvm-i8086` runtime library

`tests/i8086_codegen.rs` now links several objects (the test linker resolves symbols
across them) and runs the runtime library of `llvm-i8086/rt/` on the core.

| Command | Result |
|---|---|
| `cargo test --test i8086_codegen` | 13 of 13: the earlier 10 programs plus 32-bit divide/remainder/shifts/product (3,721 operand pairs each for the divides, every shift count 0..31), the 64-bit multiply/divide/remainder/shifts (256 pairs each, 14 shift counts), and `memcpy`/`memmove`/`memset`/`memcmp`/`bcmp` (every overlap of a 20-byte move, every byte position for `memcmp`) |
| negative controls | a deliberately wrong expectation fails for the 32-bit and the 64-bit divide |
| `cargo fmt -- --check`, `cargo clippy --all-targets` | clean |

Again a use of the core, not a change to it. It exercises `div`, `mul`, `rcl`/`rcr`,
`loop`, `jcxz`, `cwd`, `std` with `rep movsb` backward, and `repe cmpsb`; the results
are the host's. No timing is checked.


## 2026-10-25 (fourth record): compiled `llvm-i8086` code

The emulator now serves as the execution oracle for the `llvm-i8086` backend:
`tests/i8086_codegen.rs` links ELF objects built by `llc -mtriple=i8086`
(`llvm-i8086/tests/run/*.o`) with a small test linker, calls each function with the
ABI of `llvm-i8086/spec/abi.md` on the 8088 core, and compares the result with the same
computation on the host.

| Command | Result |
|---|---|
| `cargo test --test i8086_codegen` | 10 of 10 programs, 150 functions (arithmetic, comparisons, loops, shifts, extensions, globals and pointers, stack objects, calls with stack arguments, recursion, 32- and 64-bit arithmetic with carries, multiply and divide, block copies and fills); BX, SI, DI, BP and SP checked after every call |
| `cargo fmt -- --check`, `cargo clippy --all-targets` | clean |
| all other tests | unchanged and passing |

This is a use of the core, not a change to it. What it shows about the core: the
compiler's output executes with the results
the host computes, including `cbw`, `lea` into SP, shifts by CL and `[bp+si+disp]`
addressing. It does not verify timing or any device, and the linker is test code: the
backend's own linker (llvm-i8086 milestone 6) is not written.


## 2026-10-25 (third record): CGA graphics

Added: decoding of the CGA 320x200 four-colour and 640x200 two-colour modes to RGBI
pixels and PPM (`Cga::pixels`, `Cga::ppm`, `ibm_pc-emulator --ppm`), the `until idle` wait
and the `vram sha256 ... mode ... palette ...` transcript line. The RTL session now
continues from `DIR B:` into ROM BASIC graphics: `SCREEN 1,0`, a filled box
(`LINE ... ,2,BF`), a circle, `SCREEN 2` and a diagonal line (6 more steps, 20 in all).

| Command | Result |
|---|---|
| `scripts/rtl_oracle.sh` | re-recorded the 20-step session on the RTL (about 12 minutes, 448 million clocks); five graphics checkpoints with distinct video-memory hashes in modes 2Ah (320x200) and 1Eh (640x200) |
| `cargo test --release --test replay -- --ignored` | **20 of 20 steps identical to the RTL**: every screen, drive B's hash, and the SHA-256 of the 16 KiB video memory plus the mode and colour-select registers at each graphics checkpoint (1.4 s of host time) |
| CLI run diffed against the transcript | identical |
| unit tests for the decoder | banks interleave (even rows at 0, odd at 2000h), palette 0 and 1, the black-and-white bit, intensity, background colour, and 640-pixel mode |
| silicon vectors, `fmt`, `clippy -D warnings`, all other tests | unchanged and passing |

What this does and does not show: the video memory contents, written by the original
BIOS and ROM BASIC through the real INT 10h graphics routines, are byte-identical to the
RTL's; the colours the decoder assigns follow the Technical Reference's palette rules
but were checked by hand-worked unit tests and by looking at the rendered image, not
against an RTL image (the RTL has no pixel dump to compare).

## 2026-10-25 (second record): the whole 5150 on the Rust machine

Added since the first record: the board (PIC, PIT, PPI and keyboard, DMA, diskette
controller, CGA registers and text screen), the timed `Machine`, session replay, and
the `ibm_pc-emulator` command line. Same toolchain as below (Rust 1.99.1; Docker
image `ai-hw-engineer:latest` for the RTL runs). Firmware and disk: the originals
listed in `../original/provenance.md`, read from the hardware project's cache.

| Command | Result |
|---|---|
| `cargo test --release` | 1 unit + 13 CPU program + 9 device + 2 CLI + 2 replay-support tests pass; the silicon and the system replay tests are ignored without the firmware |
| `cargo test --release --test replay -- --ignored` | **14 of 14 steps identical to the RTL**: unchanged IBM BIOS POST and PC DOS 1.00 boot to the date prompt, date entry, `DIR`, `CHKDSK`, `DATE`, `BASIC.COM` and the Cassette BASIC ROM (`print 6*7`), `SYSTEM`, `FORMAT B:` (all 40 tracks through the diskette controller and DMA), `COPY TIME.COM B:CLOCK.COM`, `DIR B:`. After every step the 25-row screen matches, and the SHA-256 of drive B's image matches after FORMAT and after COPY (two different values, so the writes are real). About 1 s of host time for 335 million clocks. |
| `ibm_pc-emulator --bios ... oracle/transcripts/dos100_session.txt` diffed against the transcript | identical (the command line path, not only the test) |
| `cargo test --release --test silicon_vectors -- --ignored` | unchanged: 566,721 of 566,721 accepted and 77,256 of 77,256 extended vectors pass |
| `scripts/rtl_diff.sh 1500 7` | unchanged: 433,282 of 433,500 fuzz states agree; the 218 differences are REP-prefixed MUL/IMUL (E-A5) |
| `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` | pass |

How the RTL transcript was made: `scripts/rtl_oracle.sh` runs the hardware project's
`Vibm_pc` (the whole 5150 in RTL, same firmware and disk) through the session and
records the screen and drive-B hash after each step. The expected values come from
that run, not from this emulator.

### Findings from the bring-up

1. **Keyboard spacing.** The first full COPY replay hung: the 26-character command was
   typed instantly, the 15-key BIOS buffer overflowed, the BIOS dropped keys (including
   Enter) and beeped. The RTL host paces scan codes 25,000 clocks apart; a real keyboard
   also sends one code at a time. The Rust keyboard now does the same (E-A8). The model
   was wrong, not the firmware, and the RTL transcript exposed it.
2. **Port decode.** An early draft matched I/O ports on the wrong expression, so the
   diskette ports were silently unreachable; the BIOS then fell through to ROM BASIC.
   Caught by the DOS session (no disk activity in the FDC trace), fixed by decoding on
   the port number.
3. The unmodified BIOS passes its own POST (memory, timer, DMA, PIC, keyboard, CGA and
   diskette tests) with no firmware patches, hooks or injected state.

### Not run / not verified

- No timing is verified beyond what the BIOS and DOS observe (E-A2): instruction and
  clock counts differ from the RTL's (for example 85 million clocks to the DOS date
  prompt here, 99 million on the RTL).
- CGA graphics mode (the DONKEY.BAS regression in the hardware project), the speaker,
  and 40-column text are not exercised; the diskette controller's error paths other
  than "no disk" and "write protected" are covered by unit tests only.
- IRET's interrupt shadow is an assumption (E-A3); DRAM refresh is not simulated (E-A9).
- No whole compiled `llvm-i8086` image (a linked program with startup code) has run on the machine; individual compiled functions run in `tests/i8086_codegen.rs` (see the fourth record).

## 2026-10-25 (first record): 8088 CPU core and memory decode

Verified on Windows with Rust 1.99.1 (GNU toolchain); the RTL runs used Verilator
5.052 inside `ai-hw-engineer:latest` (Docker). Vector corpus: SingleStepTests/8086
at commit `e71c68d215a6bb8c356bd4cb3842de3bef345ca9`, read from the hardware
project's cache. RTL: `$AI_HW_ENGINEER/design/intel_8086`, unmodified.

| Command | Result |
|---|---|
| `cargo test --release` | 13 program tests and 3 template tests pass; silicon test ignored (needs corpus) |
| `cargo test --release --test silicon_vectors -- --ignored --nocapture` | **accepted: 566,721 pass, 0 fail** (the RTL's own selection is 566,721); **extended (undocumented/alias encodings): 77,256 pass, 0 fail**; 2,023 excluded (`8C`/`8E` with an undocumented segment field), same as the RTL; about 20 s |
| `docker run ... Vintel_8086 /hw/design/intel_8086/work/vectors.bin` (our all-failures runner, `rtl/tb_diff.cpp`) | the RTL on the same 566,721 vectors: **566,721 pass, 0 fail** |
| `scripts/rtl_diff.sh 1500 7` | 433,500 boundary-biased fuzz cases over 225 encodings: **433,282 agree, 218 differ**; every difference is a REP/REPNE prefix on `MUL` or `IMUL` (below) |
| `scripts/rtl_diff.sh 200 1` (after excluding out-of-contract encodings) | 57,800 cases: 57,775 agree, 25 differ, all the same class |
| `cargo fmt -- --check`, `cargo clippy --all-targets -- -D warnings` | pass |

### What the vectors found in the first draft of the core

The first run passed 566,599 of 566,721 accepted vectors. The 122 failures were
three defects, all fixed, all caught by the silicon test (so not escaped defects):

- `PUSH SP` through `FF /6` pushed the old SP instead of the decremented value.
- `AAM 0` left the flags alone; the chip sets them for a zero result before trapping.
- DAA/DAS followed the manual's pseudo-code; the chip tests the original AL against
  9Fh, adjusts original AL 9Ah..9Fh only when AF was clear, and its low-digit step
  does not set CF (ledger E-V1).

### Findings

1. **RTL vs this core and MartyPC: REP before MUL/IMUL.** This core and MartyPC
   (`external/martypc`, `cpu_808x/execute_fn.rs`: "Negate product if a REP prefix is
   present") negate the product; the RTL, which implements the negation for `IDIV`
   only, does not. The silicon corpus has no REP-prefixed `MUL`/`IMUL`, so neither
   side is proven against the chip; the behavior is undocumented. Recorded as
   assumption E-A5 and in `failure_notes/2026-10-25-rtl-8086-rep-mul-not-negated.md`.
2. **`0F` (POP CS).** The core implements it as the chip does (the corpus's 2,000
   vectors pass); the RTL rejects it by design (`fault_o`), documented in its spec.
   Not a defect; the fuzz skips it.
3. The manual's DAA/DAS pseudo-code differs from the chip (E-V1).

The core's interrupt handling (NMI/INTR priority, the STI shadow, REP interruption,
single-step) is tested by hand-worked programs, not by silicon vectors.
