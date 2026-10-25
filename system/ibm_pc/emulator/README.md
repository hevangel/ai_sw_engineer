# ibm_pc Rust system emulator

Host Rust library and headless executable for the whole IBM PC 5150: 8088 machine
code, memory and peripherals. Separate from the title reimplementations and the
`llvm-i8086` backend.

**Status:** the 8088 CPU core and the memory/I/O decode are implemented and
verified against the physical chip and the hardware project's RTL. The peripherals
(interrupt controller, timer, DMA, keyboard, diskette, CGA), the replay layer and
the CLI are not written yet; the `ibm_pc-emulator` executable is still the template
echo. See [spec/spec.md](spec/spec.md) for the contract and what is not modeled.

| Path | Contents |
|---|---|
| `src/cpu.rs` | `Cpu`: instruction-level 8088 core; `Bus` trait (byte-wide memory, I/O, NMI/INTR) |
| `src/machine.rs` | `Machine`: RAM, CGA memory, ROM, unmapped decode, I/O log, NMI/INTR lines |
| `src/lib.rs` | modules plus the template's placeholder `Emulator` (to be replaced) |
| `tests/cpu_programs.rs` | hand-assembled programs: flow, stack, interrupts, `REP`, memory map |
| `tests/silicon_vectors.rs` | the physical-chip vector corpus (ignored; needs the corpus) |
| `examples/rtl_fuzz.rs`, `rtl/tb_diff.cpp`, `scripts/rtl_diff.sh` | differential fuzz against the RTL |

From the repository root (add `+1.99.1` after `cargo` where no default toolchain is set):

```bash
cargo test --manifest-path system/ibm_pc/emulator/Cargo.toml
cargo fmt --manifest-path system/ibm_pc/emulator/Cargo.toml -- --check
cargo clippy --manifest-path system/ibm_pc/emulator/Cargo.toml --all-targets -- -D warnings

# 566,721 physical-chip vectors (needs the hardware project's cached corpus)
cargo test --release --manifest-path system/ibm_pc/emulator/Cargo.toml \
  --test silicon_vectors -- --ignored --nocapture

# differential fuzz against the RTL (needs Docker and ai-hw-engineer:latest)
RUST_TOOLCHAIN=1.99.1 system/ibm_pc/emulator/scripts/rtl_diff.sh 1500 7
```

The corpus and RTL come from a sibling checkout of `ai_hw_engineer` (`$AI_HW_ENGINEER`,
default `../ai_hw_engineer`); nothing from it is copied here. Original ROMs and disks
are not distributed; see each title's `original/provenance.md`.

- [spec/spec.md](spec/spec.md): contract, dependencies, unsupported hardware, assumption ledger
- [plans/testplan.md](plans/testplan.md): verification levels and commands
- [report/verification.md](report/verification.md): dated verification record and findings
