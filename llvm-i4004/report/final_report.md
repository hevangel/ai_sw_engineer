# llvm-i4004 final report

## Status: milestone 3b (instruction selection) — STARTED, codegen backbone not yet built

The isel milestone is scoped and its acceptance harness is in place, but the
LLVM **CodeGen backbone is not yet implemented**, so `llc -mtriple=i4004`
cannot lower IR. This section states exactly what exists and the blocker.

**Done in 3b:**
- `plans/implementation_plan.md` now carries the full 7-step isel plan, pinned
  to `abi.md` (data layout, C int = 16, i8 native on pairs, P0 scratch, P7 SSP,
  args P1–P3, P4–P6 callee-saved, 3-level hw stack then software stack).
- `scripts/isel_bcd_probe.sh` is the reproducible acceptance harness for the
  milestone function. It transcribes the BCD digit-add from
  `system/busicom_141pf/firmware/src/bcd.rs` (`add_mag`, plus an even smaller
  single-digit `bcd_add_digit`) into a standalone `no_std` function — **without
  modifying the firmware crate** — emits its LLVM IR with the pinned nightly
  rustc (LLVM 23.1.3, the same LLVM the backend is built from), swaps in the
  i4004 data layout + triple (`abi.md` §1), and runs `llc`.

**The IR extracts cleanly.** `bcd_add_digit` lowers to i8 add/sub + a compare
and an i16 pack; `bcd_add_mag` to a 16-iteration loop of i8 add/sub/compare
with a pointer load/store each step — exactly the add/sub/constants/branch/
RAM-load-store subset the isel plan targets first.

**Blocker (honest):** `llc -mtriple=i4004` reports

```
llc: error: unable to get target for 'i4004', see --version and --triple.
```

because the installed toolchain registers only the **MC layer + disassembler**,
not a CodeGen `TargetMachine`. `LLVMInitializeI4004Target` is still the MC-only
stub that registers no TargetMachine (by design through milestone 3a). Making
`llc` work requires the full backbone — `I4004TargetMachine` (on LLVM 23.1's
`CodeGenTargetMachineImpl` + `buildCodeGenPipeline`), `I4004Subtarget`,
`I4004RegisterInfo`/`I4004InstrInfo` CodeGen classes, `I4004FrameLowering`,
`I4004ISelLowering` (legalize i8, expand i16/i32, lower ret/add/sub/setcc/
br/load/store, and the AND/OR/XOR expansion the 4004 lacks), `I4004CallingConv`,
`I4004ISelDAGToDAG`, `I4004AsmPrinter` + `MCInstLower` — roughly 18 inter-
dependent files that must all compile together against LLVM 23.1's exact APIs.
That is a multi-day backend bring-up; it was **not** completed in this session,
and a half-written, non-compiling backbone was deliberately **not committed**
(it would break the shared LLVM build for the whole repo, including the
concurrent firmware worktree).

**Where this stops:** at a reproducible IR → `llc` probe that pinpoints the
missing TargetMachine. The next session resumes by building the backbone in the
smallest-first order in the plan, re-running `isel_bcd_probe.sh` after each
piece until it emits assembly our `llvm-mc` accepts, then reporting the BCD
`.text` byte size. No rustc custom-target integration until then (plan step 7).

---

# llvm-i4004 final report

## Status: milestone 3a (disassembler + independent validation) complete

Milestone 2 (MC layer) is complete and milestone 3a adds the **disassembler**,
the **independent encoding cross-check** against two outside decoders, and the
**R_I4004_FIN8** relocation test. `llvm-mc -triple=i4004 --disassemble` and
`llvm-objdump -d` now decode all 46 instructions, and an assemble ->
disassemble -> reassemble round trip is byte-identical. Instruction selection /
codegen is the next milestone (3b/4), tracked on branch `llvm/i4004-isel`.

### Milestone 3a results

| Check | Result |
|---|---|
| lit suite (`tests/mc/`, incl. `tests/mc/disasm/`) | **16 / 16 pass** |
| Disassembler coverage | all 46 opcodes, both 1- and 2-byte forms, all 16 JCN conditions, all 8 pairs |
| Round trip (asm -> obj -> disasm -> re-asm -> obj) | **byte-identical** across 52 instruction instances (`disasm/roundtrip.s`) |
| Independent cross-check | 256 first bytes + 256 JCN second bytes vs **external/chips** AND **ai_hw_engineer RTL**: **no disagreements** |
| Undefined opcodes 0xFE/0xFF, 0x01-0x0F | decoder refuses (invalid-encoding warning, no mnemonic), matching `isa.md` §8 |
| `R_I4004_FIN8` relocation | now emitted and asserted via `.reloc` (`tests/mc/reloc_fin8.s`); all four `R_I4004_*` target relocs are `.reloc`-nameable |

### Disassembler design

`target/lib/Target/I4004/Disassembler/I4004Disassembler.cpp` is a **hand-written
variable-length decoder**, not a TableGen `-gen-disassembler` table: the MCS-4
stream mixes 1- and 2-byte instructions and TableGen's fixed-length decoder
assumes a single width. The decoder dispatches on the OPR high nibble, one
`case` per `isa.md` row, reads a second program byte for JCN/FIM/JUN/JMS/ISZ,
and returns `Fail` (Size=1) on a truncated two-word instruction or an undefined
opcode so a caller can resynchronise. CMake auto-registers it via
`Disassemblers.def` once the `Disassembler/` subdir exists (a clean reconfigure
is needed the first time).

### Independent cross-check — the headline validation

`scripts/crosscheck_disasm.py` runs OUR disassembler over every first byte
0x00-0xFF (and all 256 second bytes of a representative two-word opcode) and
compares each decode against **two decoders written independently of this
backend**, both read at run time and never copied into the repo:

1. `external/chips/src/mcs4/cpu4004.rs` — V. Ilmer's / A. Weissflog's
   behavioural Rust emulator (MIT/zlib). The script also asserts the chips
   source still has the OPR=2/3 low-bit split and the 0xE/0xF group arms, so
   the oracle tracks that file rather than a stale assumption.
2. `../ai_hw_engineer/design/intel_4004/src/intel_4004.sv` — the synthesizable
   RTL. The script parses its `Opr*` localparams and confirms each maps its
   nibble to the canonical group before applying the shared dispatch.

**Result: zero disagreements** against both oracles. Note this directly
supersedes the milestone-2 report's claim that the second-oracle cross-check
was "unavailable in this environment": `ai_hw_engineer` **is** present on this
host (`B:\ai_hw_engineer`) and the `external/chips` submodule checks out
cleanly, so both oracles were used. The earlier "unavailable" note was wrong.

During harness development the script first reported 17 false "findings"
(0x01-0x0F, 0xFE, 0xFF) — these were a *harness* bug (a padding byte let the
single-byte probe resynchronise onto a trailing NOP), not a decoder bug; the
decoder was independently confirmed to reject those bytes. Fixed by probing
single-byte opcodes with exactly one byte. This is recorded so the green result
is not mistaken for a harness that cannot fail.

---

## Status: milestone 2 (MC layer) complete

`llvm-mc -triple=i4004 -show-encoding` assembles **all 46 Intel 4004
instructions** to the manufacturer opcode table in `spec/isa.md`; the object
writer stamps the private `e_machine 0x4004` and emits the `R_I4004_*`
relocations from `spec/abi.md`. No instruction selection / codegen yet
(milestone 3).

| Check | Result |
|---|---|
| MC encode / error / reloc / ELF tests | **10 / 10 pass** (`tests/mc/`) |
| Instructions assembled (`-show-encoding`) | **46 / 46** opcodes, bytes match `isa.md` |
| ELF `e_machine` | `EM_I4004 (0x4004)`, `elf32-i4004`, ELFCLASS32, LSB |
| Relocations | `R_I4004_ADDR12`, `R_I4004_PAGE8`, `R_I4004_IMM8` emitted for unresolved JUN/JMS/JCN/ISZ/FIM |
| Codegen tests | n/a (milestone 3) |
| Smoke programs on simulator | n/a (milestone 3+) |
| Titles built with this backend | none yet |

### Test breakdown (`llvm-i4004/tests/mc/`)

| File | Covers |
|---|---|
| `basic.s` | the 16 `OPR=0x0..0xD` rows (NOP, JCN, FIM, SRC, FIN, JIN, JUN, JMS, INC, ISZ, ADD, SUB, LD, XCH, BBL, LDM) |
| `ram_rom.s` | the 16 `OPR=0xE` rows (`wrm`=0xE0 .. `rd3`=0xEF) |
| `accgroup.s` | the 14 defined `OPR=0xF` rows (`clb`=0xF0 .. `dcl`=0xFD) |
| `jcn_cond.s` | JCN condition mnemonics (Z/C/T/NZ/NC) + hex nibble + `jmp` alias |
| `pairs.s` | all 8 pair encodings for FIM/SRC/FIN/JIN + `rNrM` aliases |
| `reloc.s` | symbolic two-word operands -> `fixup_addr12` / `fixup_page8` |
| `elf.s` | `-filetype=obj` + `llvm-readobj`: `e_machine 0x4004` + `R_I4004_*` |
| `errors/err_imm_range.s` | `ldm 16`, `bbl 16`, `fim p0, 256` rejected |
| `errors/err_pair_vs_reg.s` | `src r3`, `add p2`, `fin r5` rejected |
| `errors/err_undefined.s` | undefined `0xFE/0xFF` have no mnemonic; bad mnemonics rejected |

## Build

- **LLVM**: `external/llvm-project` submodule pinned to **`llvmorg-23.1.3`**
  (shallow). This is the LLVM version of the repo's pinned nightly rustc
  (`rustc +nightly-2026-10-15 -vV` -> `LLVM version: 23.1.3`), verified in the
  container. The tag exists, so no fallback was needed.
- **Overlay, not in-place edits**: the 4004 target lives in
  `llvm-i4004/target/` and is applied onto a scratch copy of the submodule by
  `scripts/apply_overlay.py` (idempotent, anchored string edits to Triple /
  ELF / ELFDumper infra; the submodule itself is never modified).
- **Command** (from the repo root on the Windows host):

  ```
  docker run --rm --user root \
    -v B:\ai_sw_engineer_mc:/workspace \
    -v ai-sw-llvm-build:/build \
    -w /workspace ai-sw-engineer:latest \
    bash -lc 'chown -R mos:mos /build; su mos -c "llvm-i4004/scripts/build_llvm.sh"'
  ```

  The LLVM source copy and the build tree live in the Docker **named volume**
  `ai-sw-llvm-build` (`/build`), not on the slow Windows bind mount. Built with
  `LLVM_EXPERIMENTAL_TARGETS_TO_BUILD=I4004`, `LLVM_TARGETS_TO_BUILD=""`,
  `CMAKE_BUILD_TYPE=Release`, `LLVM_ENABLE_ASSERTIONS=ON`, Ninja, host `gcc`
  (the image's `clang` defaults to the `mos` bare-metal target), `-j16`.
- **Tools built & installed** into `llvm-i4004/install/bin` (gitignored):
  `llvm-mc`, `llvm-tblgen`, `FileCheck`, `not`, `count`, `llvm-readobj`,
  `llvm-objdump`.
- **Build time**: the first clean build (tblgen + the minimal LLVM MC/Support
  stack + the I4004 libraries + the seven tools) is roughly **12-20 min** at
  `-j16` on the 32-core host; a no-op incremental re-run (overlay + ninja
  up-to-date check) is **~38 s**. The one-time source copy from the Windows
  bind mount into the volume adds several minutes and is cached thereafter.
- **Run the tests**:

  ```
  docker run --rm -v B:\ai_sw_engineer_mc:/workspace -w /workspace \
    ai-sw-engineer:latest bash -lc 'llvm-i4004/scripts/run_tests.sh'
  ```

## What the MC layer contains

`llvm-i4004/target/lib/Target/I4004/`:

- `I4004.td`, `I4004RegisterInfo.td`, `I4004InstrFormats.td`,
  `I4004InstrInfo.td` - registers (ACC, CY, R0-R15 / GR4, P0-P7 / PR8) and one
  TableGen record per `isa.md` row with the exact `OPR:OPA` encoding.
- `MCTargetDesc/` - `I4004MCCodeEmitter` (byte emission + fixups),
  `I4004AsmBackend` (fixup kinds, relocation hand-off), `I4004ELFObjectWriter`
  (`e_machine 0x4004`, `R_I4004_*`), `I4004InstPrinter`, `I4004MCAsmInfo`,
  registration glue.
- `AsmParser/I4004AsmParser.cpp` - register / pair / JCN-condition parsing,
  operand-range validation.
- `TargetInfo/` + a minimal `I4004TargetMachine.cpp` stub so the target
  resolves as a proper LLVM component (no code generator yet).

## Known gaps

- **No instruction selection / codegen** (milestone 3): `I4004TargetMachine` is
  a stub providing only the component init hook; `llc` cannot lower IR yet.
- **Page-local cross-page checking** (`isa.md` sec.9 end-of-page quirk) is
  **not yet enforced** by the assembler for the `fixup_page8` / `fixup_fin8`
  relocations. The relocations are emitted correctly; the linker-side page
  identity check is deferred to the linker/image-packer milestone (5). An
  `err_page_cross.s` test is therefore not yet included (it would require that
  check or `ld.lld` placement to fail). Tracked here and in `plans/testplan.md`.
- **FIN pair-0 relocation** (`R_I4004_FIN8`) is defined but not yet exercised
  by a test (no `.word`/FIN constant-table assembly path until codegen needs
  it).
- **Second-oracle cross-check unavailable in this environment**: the task asks
  to cross-check encodings against `B:\ai_hw_engineer\design\intel_4004` and
  the `external/chips` MCS-4 emulator. `ai_hw_engineer` is outside this
  worktree and the `external/chips` submodule is not checked out here, so the
  only oracle used was the `spec/isa.md` manufacturer opcode table (itself
  sourced from the MCS-4 manual and cross-validated against MAME per its own
  sources sec.10). All 46 encodings match that table byte-for-byte.
- **ELFDumper / Triple / ELF infra edits** are applied by the overlay script,
  not committed into the submodule. They are small and idempotent; if the
  pinned tag ever moves, re-verify the anchors in `scripts/apply_overlay.py`.

## Spec corrections / findings

- No `isa.md` / `abi.md` corrections were needed: the opcode table and
  relocation list were complete and the implemented encodings matched them
  exactly on the first passing build.
- **Finding (toolchain):** the `ai-sw-engineer:latest` image's `/usr/local/bin/clang`
  defaults to `mos-unknown-unknown`, so building host LLVM with it fails at the
  CMake compiler check (it tries to link an MCS-6502 `crt0.o`). The backend
  build therefore uses host `gcc`/`g++`. Documented in `build_llvm.sh`.
- **Finding (Windows checkout):** git on the Windows host checks the LLVM
  submodule out with CRLF line endings, which breaks the `/bin/sh` helper
  scripts cmake executes (`config.guess`). `build_llvm.sh` strips CR from those
  scripts in the scratch copy before configuring.
- **Finding (LLVM 23.1 TableGen):** `-gen-instr-info` now requires every target
  to remap the standard `ptr_rc` pseudo-instructions onto a concrete register
  class via `RemapAllTargetPseudoPointerOperands<...>`, even for an MC-only
  target. Done with `PR8` (the pointer-capable pair class).
