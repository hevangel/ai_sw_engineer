# llvm-i4004 final report

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
