# ibm_pc Rust system emulator

Host Rust library and command-line tool for the whole IBM PC 5150: an 8088 core,
the planar's memory and I/O decode, and the interrupt controller, timer, PPI and
keyboard, DMA controller, diskette controller and CGA. Separate from the title
reimplementations and the `llvm-i8086` backend.

**Status:** boots the unchanged IBM BIOS, loads PC DOS 1.00 from a 160 KiB diskette image,
and runs typed DOS and BASIC sessions. The screens, and the written drive B image, match
the hardware project's RTL for a 20-step session (boot, date, `DIR`, `CHKDSK`, ROM and
disk BASIC, `FORMAT B:`, `COPY`, and BASIC drawing in both CGA graphics modes, compared by
video-memory hash). The CPU core passes all 566,721 physical-chip vectors
the RTL passes. Not done: the speaker, an interactive graphical front end, character-ROM
text rendering, and compiled `llvm-i8086` images (the backend has no code yet). Contract and limits:
[spec/spec.md](spec/spec.md); evidence: [report/verification.md](report/verification.md).

| Path | Contents |
|---|---|
| `src/cpu.rs` | `Cpu`: instruction-level 8088 core; `Bus` trait (byte-wide memory, I/O, NMI/INTR) |
| `src/machine.rs` | `Machine`/`Board`: memory map, I/O decode, clock, interrupt wiring, DMA service |
| `src/pic.rs`, `pit.rs`, `ppi.rs`, `dma.rs`, `fdc.rs`, `cga.rs` | the peripherals; `ppi.rs` also holds the keyboard; `cga.rs` also decodes graphics modes to pixels and PPM |
| `src/session.rs`, `keys.rs`, `sha256.rs` | session scripts and transcripts, text to scan codes, image fingerprints |
| `src/main.rs` | the `ibm_pc-emulator` command line |
| `tests/` | `cpu_programs`, `devices`, `cli`, `replay` (RTL transcripts), `silicon_vectors` (physical-chip corpus) |
| `oracle/transcripts/` | screens and drive hashes recorded from the RTL (`scripts/rtl_oracle.sh`) |
| `examples/rtl_fuzz.rs`, `rtl/tb_diff.cpp`, `scripts/rtl_diff.sh` | CPU differential fuzz against the RTL |
| `examples/bringup.rs` | boot the ROMs and print the screen, for bring-up |
| `original/provenance.md` | the copyrighted firmware and disk the tests read, with SHA-256s |

## Run

The original BIOS, Cassette BASIC and PC DOS 1.00 disk are not in this repository.
`$IBM_PC_ROMS` (or `$AI_HW_ENGINEER/system/ibm_pc/work/reference`) points at a directory
with `bios.bin`, `basic.bin`, `pcdos100.img` and `workdisk.img`
([provenance](original/provenance.md)). From the repository root, with
`R=../ai_hw_engineer/system/ibm_pc/work/reference` (add `+1.99.1` after `cargo` where no
default toolchain is set):

```bash
cargo build --release --manifest-path system/ibm_pc/emulator/Cargo.toml
system/ibm_pc/emulator/target/release/ibm_pc-emulator \
  --bios $R/bios.bin --basic $R/basic.bin --disk-a $R/pcdos100.img --disk-b $R/workdisk.img \
  system/ibm_pc/emulator/oracle/transcripts/dos100_session.txt > /tmp/session.out
diff <(grep -v '^#' /tmp/session.out) <(grep -v '^#' system/ibm_pc/emulator/oracle/transcripts/dos100_session.txt)
```

Add `--ppm screen.ppm` to save the final screen as an image when it ends in a CGA graphics mode.

The session file is the input (its `screen` and `disk` lines are ignored) and the output
is a transcript in the same format; an empty diff means the Rust machine showed what the
RTL showed. Write your own session with `boot`, `type <text>\r` and `until contains|endswith|row <text>` lines.

## Verify

```bash
cargo test --manifest-path system/ibm_pc/emulator/Cargo.toml
cargo fmt --manifest-path system/ibm_pc/emulator/Cargo.toml -- --check
cargo clippy --manifest-path system/ibm_pc/emulator/Cargo.toml --all-targets -- -D warnings

# against the RTL's recorded session (needs the firmware, about a second)
cargo test --release --manifest-path system/ibm_pc/emulator/Cargo.toml --test replay -- --ignored

# 566,721 physical-chip vectors (needs the hardware project's cached corpus)
cargo test --release --manifest-path system/ibm_pc/emulator/Cargo.toml \
  --test silicon_vectors -- --ignored --nocapture

# CPU differential fuzz against the RTL (needs Docker and ai-hw-engineer:latest)
RUST_TOOLCHAIN=1.99.1 system/ibm_pc/emulator/scripts/rtl_diff.sh 1500 7

# re-record the RTL's session (about 10 minutes; needs the RTL system build)
system/ibm_pc/emulator/scripts/rtl_oracle.sh
```

The corpus, the RTL and the firmware cache come from a sibling checkout of `ai_hw_engineer`
(`$AI_HW_ENGINEER`, default `../ai_hw_engineer`); nothing from it is copied here.

- [spec/spec.md](spec/spec.md): contract, dependencies, unsupported hardware, assumption ledger
- [plans/testplan.md](plans/testplan.md): verification levels and commands
- [report/verification.md](report/verification.md): dated verification records and findings
