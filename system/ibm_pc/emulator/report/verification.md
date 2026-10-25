# ibm_pc emulator verification

## 2026-10-25: 8088 CPU core and memory decode

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

### Not run

The peripherals, keyboard, diskette and CGA do not exist yet, so no BIOS POST, DOS
boot or application has run on the machine model, and the CLI is still the
template echo. RTL system-level (`system/ibm_pc`) and MartyPC whole-machine
comparisons wait for that. The core's interrupt handling (NMI/INTR priority, the
STI shadow, REP interruption, single-step) is tested by hand-worked programs, not
by silicon vectors; IRET's interrupt shadow is an assumption (E-A3). No timing is
verified (E-A2).
