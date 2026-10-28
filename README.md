# AI Software Engineer

> An open-source experiment in asking frontier models to reimplement historically significant software in Rust, and to build the compiler backends that put that Rust back on the original chips.

## What this repository is

This is the software-side sibling of [ai_hw_engineer](https://github.com/hevangel/ai_hw_engineer). That project recreates historical chips as verified RTL. This one takes the software those chips ran (calculator firmware, monitors, BASIC interpreters, BIOSes, games) and asks:

> Can a frontier model rewrite that software in modern Rust, then build the LLVM backend, assembler and linker needed to compile the Rust back onto a 4004, an 8008, a 6502 or an 8086, so that the result runs on the original machine?

**This repository is the AI software engineer.** Its specs, plans, tool docs, templates, scripts, Rust sources, compiler backends and reports are the working context the engineer uses.

The project has two products besides the reimplemented software:

- **A compiler backend per chip family** (`llvm-<chip>/`) that lowers Rust to the original ISA.
- **A host Rust emulator per complete system** (`system/<system>/emulator/`): a library and command-line tool that run original and compiled machine-code images with the machine's CPU, memory, keyboard and display or printer, and replay deterministic inputs. An LLVM backend or a host-only rewrite does not replace it. The first one is the [Busicom 141-PF library and CLI](system/busicom_141pf/emulator/README.md); every new system starts from the emulator template in `system/_template/emulator/`.

## The pipeline

Every piece of software goes through the same pipeline:

```text
 original binary ──► chip simulator ──► oracle traces / I/O transcripts
                                              │
 reference docs ──► Rust reimplementation ────┤  (1) host run: cargo test vs oracle
                          │                   │
                          ▼                   │
                 rustc --emit=llvm-ir         │
                          │                   │
                          ▼                   │
          llvm-<chip>: llc → asm → assembler/linker → ROM/disk image
                                              │
                          ▼                   │
          chip simulator (ai_hw_engineer RTL / ISS / 3rd-party emulator)
                          │                   │
                          └──────── (2) target run: compare vs oracle
```

1. Run the original software in a simulator of the real chip and record oracle behavior (key-press transcripts, printer/screen output, memory dumps, instruction traces).
2. Reimplement the software in `no_std` Rust from the reference material, behavior first, and prove it on the host against the oracle.
3. Lower the Rust through LLVM IR to the target ISA with the chip's backend in `llvm-<chip>/`.
4. Assemble, link and package the image within the real machine's ROM/RAM limits.
5. Run the image on the chip simulator (preferably the RTL from `../ai_hw_engineer`) and compare against the same oracle.
6. Investigate failures in the Rust, the backend or the simulator instead of stepping around them.
7. Report what works, what does not fit, and what was learned.

In parallel, the system's Rust emulator runs the original and the compiled images from the same input transcripts. When an emulator shares a chip core with a reference adapter it is not an independent check, so it is cross-checked against the RTL or another implementation.

A passing host test is useful evidence. It is not proof the program fits in 4 KiB of 4001 ROM.

## Repository structure

```text
ai_sw_engineer/
├── README.md, AGENTS.md, CONTRIBUTING.md, LICENSE
├── spec/                   — Rules for the agent, one topic per file (AGENTS.md indexes them)
├── reference/              — Shared reference documents (datasheets, manuals, ISA refs)
├── docs/                   — Tool documentation written for the agent (Rust, LLVM, simulators)
├── system/                 — One folder per platform, one subfolder per software title
│   ├── README.md           — System index
│   ├── _template/          — Platform + emulator + software templates
│   └── <system>/           — platform/, emulator/ (host Rust system emulator), <software>/
├── llvm-_template/         — Starting point for a new chip backend
├── llvm-<chip>/            — One LLVM backend (and Rust target) per chip family
├── external/               — Git submodules: llvm-mos, rust-mos, emulators, test suites
├── scripts/                — Project-level utilities (scaffolding, toolchain checks)
├── failure_notes/          — Post-mortems for bugs that escaped verification
└── session_history/        — Per-session engineering logs
```

See [system/README.md](system/README.md), [llvm-_template/README.md](llvm-_template/README.md) and [external/README.md](external/README.md) for the per-area conventions.

### How the agent is instructed

[AGENTS.md](AGENTS.md) is a short entry point: role, mission and the rules that always apply. The detailed rules are split by topic in [spec/](spec/README.md) (mission, repository layout, Rust, LLVM backends, simulators and emulators, original-software copyright, documentation, verification, escaped defects, git workflow). The agent reads a file only when its task matches, and this README and the human-facing docs are updated whenever those rules change in a way a reader would notice. The same files are the working agreement for human reviewers.

### Planned systems

| System | CPU | LLVM backend | Status |
|---|---|---|---|
| Busicom 141-PF calculator | Intel 4004 | `llvm-i4004` (new) | **runs on the 4004**: Rust firmware (with hand-written BCD and driver routines in `asm/`) compiles through `llvm-i4004` to a flat **4,095 B** ROM (4,096 B max, 3.2× the 1,280 B original) and prints the original ROM's tape **42/42 on the ai_hw_engineer RTL and on external/chips**; host tests 42/42 — see `system/busicom_141pf/firmware/report/final_report.md` |
| Micral N | Intel 8008 | `llvm-i8008` (new) | **runs on the 8008 RTL**: boot ROM, MIC 01 monitor (in its 2 KiB ROM), Shooting Stars and SCELBAL BASIC rewritten in Rust; the last two match the originals byte for byte — see `system/micral_n/README.md` |
| Apple II / IIe | MOS 6502 | `llvm-mos6502` (llvm-mos) | planned |
| Atari 2600 | MOS 6507 | `llvm-mos6502` (llvm-mos) | planned |
| Commodore 64 | MOS 6510 | `llvm-mos6502` (llvm-mos) | planned |
| Nintendo NES | Ricoh 2A03 | `llvm-mos6502` (llvm-mos) | planned |
| IBM PC 5150 / XT | Intel 8088 / 8086 | `llvm-i8086` (new, real-mode 8086 only) | **Rust runs on the 8088**: `i8086-unknown-none` rustc target, CHKDSK reimplemented in Rust (identical to the original on 24 test disks); the emulator boots the original BIOS and PC DOS 1.00 and matches the RTL; compiler profile and verification in `system/ibm_pc/toolchain/` — see `system/ibm_pc/README.md` |
| IBM PC AT 5170 | Intel 80286 | `llvm-i286` (new) | audit/specification stage (`llvm-i286/`, issue #66) |

## Quick start

```bash
git clone --recurse-submodules https://github.com/hevangel/ai_sw_engineer.git
# or, in an existing checkout (large submodules are shallow):
git submodule update --init --depth 1

./scripts/check_toolchain.sh                    # what is installed, what is missing
./scripts/new_system.sh apple_ii mos6502        # new platform + system emulator crate
./scripts/new_software.sh apple_ii integer_basic   # new software title
./scripts/new_llvm_target.sh i4004              # new LLVM backend

cd system/apple_ii/integer_basic
./scripts/run_all.sh                            # host test → IR → target image → simulator compare
```

The chip simulators come from a sibling checkout of `ai_hw_engineer`; set `AI_HW_ENGINEER=/path/to/ai_hw_engineer` if it is not at `../ai_hw_engineer`.

## Scope and limitations

This project does not claim that:

- A Rust reimplementation is byte- or cycle-identical to the original program (unless a report says it was checked).
- A host test proves the target build is correct; the compiler backend is a second, independent source of bugs.
- Every title will fit the original memory map; when it does not, the report says by how much and why.
- A system emulator is cycle-accurate or models every board variant; each emulator's `spec/spec.md` lists what it does not implement.
- Original ROMs and disk images are redistributable. Copyrighted originals stay out of git; only their hashes and provenance are recorded.

## License

Project-owned material is licensed under the [MIT License](LICENSE). Submodules under `external/` and reference material under `reference/` keep their own licenses (LLVM and llvm-mos: Apache-2.0 WITH LLVM-exception; Rust: MIT/Apache-2.0).
