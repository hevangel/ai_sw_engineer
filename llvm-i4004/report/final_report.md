# llvm-i4004 final report

## Status: milestone 3b (instruction selection) — CodeGen BACKBONE BUILT

`llc -mtriple=i4004` now lowers LLVM IR to real MCS-4 assembly our own
`llvm-mc` accepts. The backbone is a minimal-but-real target built on LLVM
23.1's `CodeGenTargetMachineImpl` (legacy pass-config path): `I4004TargetMachine`,
`I4004Subtarget`, `I4004RegisterInfo`/`I4004InstrInfo` CodeGen classes,
`I4004FrameLowering`, `I4004ISelLowering`, `I4004CallingConv.td`,
`I4004ISelDAGToDAG`, `I4004AsmPrinter` + `I4004MCInstLower`, and a post-RA
`I4004ExpandPseudos` pass. The data layout is the authoritative `abi.md` §1
string set explicitly on the TargetMachine; the overlay registers the i4004
16-bit pointer width in `Triple.cpp`.

### What compiles today (verified)

| Construct | Lowering | Verified |
|---|---|---|
| empty fn | — | `llc -filetype=null` accepts |
| `ret void` / `ret i8 c` | `FIM p1, c` (i8 in P1, abi.md sec.4.2) + `BBL 0` | lit + exec |
| `ret i16` | split to two i8 (P1:P2) | lit (`elf_machine`), compiles |
| `add i8` | `CLC; LD; ADD; XCH` per nibble, carry chained | **exec 10/10 on chips** |
| `sub i8` | `CLC; LD; SUB; XCH` low, `CMC`, then high nibble | **exec 10/10 on chips** |
| args (i8) | P1, P2, P3 then RAM-stack slot (abi.md sec.4.1) | lit + exec |
| pair copy | `LD/XCH` x2 (`copyPhysReg` -> `COPYpr`) | compiles, used by RA |
| unconditional `br` | `JUN <mbb>` (addr12 reloc resolves the label) | lit |
| ELF object | `-filetype=obj` carries `e_machine 0x4004` | lit |
| MC round-trip | `llc` `.s` re-assembled by independent `llvm-mc` | lit |

Multiply/divide/shift are set to libcalls (`__mulqi3`, `__udivqi3`, ... into a
future `libi4004rt`, abi.md sec.6). AND/OR/XOR are `Expand` (the 4004 has no
native bitwise ALU).

### Verification (in the ai-sw-engineer container)

- **lit/FileCheck: 22/22** — 16 MC tests + 6 CodeGen tests
  (`tests/codegen/{ret_const,add_i8,sub_i8,branch,elf_machine,mc_roundtrip}.ll`).
- **Execution on external/chips: 20/20** — `scripts/exec_fn.py` compiles
  `add8`/`sub8` with `llc`, runs the compiled MCS-4 code on chips' real
  `mcs4::Board` (`tools/chips_xcheck/run_fn`) for 10 input pairs each, and
  compares against the host. This is an EXECUTION test, not FileCheck: it caught
  and fixed a real backend bug — the i8 subtract borrow chain was wrong on every
  case (both an STC-once and a CLC-once form failed; the correct form is CLC
  then CMC between nibbles, because the 4004 SUB uses `ACC + ~R + ~CY` so the
  carry-out polarity must be flipped between nibbles).
- MC differential disassembler cross-check on chips still green (unchanged).

Both lit and the execution test are wired into `scripts/run_tests.sh`.

### Honest blockers (remaining to run the firmware)

The yardstick `scripts/isel_bcd_probe.sh` (the BCD digit-add) does **not** yet
compile. The probe advances past target lookup and `br`, then stops at:

```
LLVM ERROR: Do not know how to expand this operator's operand!
```

Isolated to two missing layers (minimal IR reproduces each):

1. **Loads/stores to 4002 RAM** (`load i8, ptr` / `store`). addrspace-0 memory
   access via `SRC` + `RDM`/`WRM` (two nibbles per byte) is not lowered yet;
   the exact AS-0 pointer bit-packing is abi.md §7 open-question 2. This is the
   gateway to any firmware code that touches memory (the BCD `a[i]` accesses).
2. **Comparisons / conditional branches** (`icmp` + `br i1`). `SETCC` to the
   4004's single `ACC==0` / `CY` test and `BR_CC` lowering are not built; the
   `BRZ`/`BRNZ` pseudos and their `JCN` expansion are in place awaiting the
   `SETCC` lowering that feeds them.

Not started beyond those: i16/i32 expansion to byte ops, the `libi4004rt`
mul/div/shift helpers, calls deeper than a leaf with the software return stack,
a linker/ROM image packer, and rustc custom-target integration (plan step 7).
So there is still **no compiled firmware image and no real BCD `.text` size**;
the original ROM is 1280 bytes (4096 max) and that comparison must wait until
loads/stores + compares land and `isel_bcd_probe.sh` emits assembly.

### Where this stops

At a verified arithmetic/control-flow/return/call-ABI backbone with lit +
on-silicon-model execution tests, and a BCD probe that pinpoints the two
remaining selection layers (memory + compares). The next session resumes by
implementing addrspace-0 load/store (SRC+RDM/WRM) and SETCC/BR_CC, re-running
`isel_bcd_probe.sh` after each, then reporting the BCD `.text` byte size.

---

# llvm-i4004 final report (milestone 3a, historical)

## Status: milestone 3a (disassembler + independent validation) complete

Milestone 2 (MC layer) is complete and milestone 3a adds the **disassembler**,
a **differential encoding cross-check that executes every encoding on an outside
emulator**, and the **R_I4004_FIN8** relocation test. `llvm-mc -triple=i4004
--disassemble` and `llvm-objdump -d` now decode all 46 instructions, and an
assemble -> disassemble -> reassemble round trip is byte-identical. Instruction
selection / codegen is the next milestone (3b/4), tracked on branch
`llvm/i4004-isel`.

### Milestone 3a results

| Check | Result |
|---|---|
| lit suite (`tests/mc/`, incl. `tests/mc/disasm/`) | **16 / 16 pass** |
| Disassembler coverage | all 46 opcodes, both 1- and 2-byte forms, all 16 JCN conditions, all 8 pairs |
| Round trip (asm -> obj -> disasm -> re-asm -> obj) | **byte-identical** across 52 instruction instances (`disasm/roundtrip.s`) |
| Differential disassembler cross-check | every encoding **executed on external/chips** and the observed effect matched the effect predicted from OUR disassembly: **no disagreements** (336 data/ALU/IO assertions + 17 control-transfer landings). See §"Differential cross-check" below. RTL half not run (no simulator here). |
| Undefined opcodes 0xFE/0xFF, 0x01-0x0F | decoder refuses (invalid-encoding warning, no mnemonic); executed on chips as a cross-check — all are NOP-equivalent **except 0xFF, which makes chips panic** (finding below) |
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

### Differential cross-check — the headline validation (corrected)

> **Correction to the original milestone-3a claim.** The first version of this
> section (and `scripts/crosscheck_disasm.py` as merged in PR #8) claimed the
> disassembler "matches two independent decoders (external/chips AND the
> ai_hw_engineer RTL), zero disagreements". **That claim was overstated.** The
> old script did not execute anything and did not compare operands. It compared
> llvm-mc's *mnemonic* against `oracle_dispatch()` — a hand-written Python
> opcode table *in that same script* — and merely regex-checked that the chips
> source and the RTL source still contained a few expected tokens / localparams.
> Register numbers, pairs, JCN conditions, immediates and addresses were never
> checked, and neither chips nor the RTL was ever run. It was, in effect, our
> own table checked against itself. The text below describes what replaced it.

The real check now lives in the Rust tool `tools/chips_xcheck` (driven by
`scripts/crosscheck_disasm.py`, wired into `run_tests.sh`). For every 4004
encoding it:

1. disassembles the bytes with OUR `llvm-mc --disassemble` (mnemonic **and**
   operands);
2. predicts the full machine effect from THAT disassembly using the MCS-4
   manual semantics in `spec/isa.md`, **parameterised by the operands we
   printed** (e.g. `add r3` predicts `ACC ← ACC + R3 + CY`, `fim p5, 0x2A`
   predicts `R10 ← 2, R11 ← A`);
3. **EXECUTES the same bytes on `external/chips`** (its real `mcs4::Board`;
   read & run only, never copied) and recovers the resulting state —
   accumulator, carry, all 16 index registers, RAM/ROM port and character
   writes — through the chip's own instructions (a short readout program that
   copies hidden CPU state into 4002 RAM, which chips exposes publicly);
4. asserts prediction == observation. Control transfers (JUN/JMS/JCN/ISZ/JIN)
   are checked with a landing harness: a marker on the fall-through path and a
   marker at the branch target reveal which path chips actually took.

Because the expected value comes from EXECUTING chips, a wrong operand field or
a swapped mnemonic in our disassembler makes step 2 disagree with step 3 — a
genuine finding. This is verified by two mutation tests (below).

**Coverage.** 336 data/ALU/IO effect assertions (every defined first byte, plus
operand-field sweeps: all 16 register indices for INC/ADD/SUB/LD/XCH, all 16
immediates for LDM/BBL, all 8 FIM pairs over a spread of data bytes) and 17
control-transfer landings (JUN, JMS, the full 16-row JCN condition matrix over
ACC/CY, ISZ with zero/non-zero results, JIN). **Result: no disagreements** —
every encoding our disassembler decoded, executed on chips, behaves exactly as
our mnemonic+operands predict.

**Mutation sanity (the check can fail).** With `MUTATE=swap_acc` (our printer
emits `iac`↔`dac` swapped, i.e. two accumulator-group ops transposed) the check
reports exactly the 2 expected findings — chips increments where we predicted a
decrement and vice-versa. With `MUTATE=reg_off1` (every register operand
printed off by one) it reports 160 findings across the register sweeps. Both
revert cleanly to zero. So the green result is not a harness that cannot fail.

**What is proven vs not:**

- *Proven:* for every DEFINED encoding, our disassembler's mnemonic **and
  operands** correctly describe what the chips 4004 emulator actually does to
  the accumulator, carry, named register(s), and RAM/ROM ports. Operand
  correctness (register index, pair, JCN condition selection, immediate value,
  branch target) is executed, not assumed.
- *Not proven here:* the **ai_hw_engineer RTL half**. The brief asks to also run
  each encoding on `B:\ai_hw_engineer\design\intel_4004` and treat chips-vs-RTL
  disagreements as findings. The RTL source (`src/intel_4004.sv`) is present,
  but **no Verilog simulator is available** — neither `verilator` nor
  `iverilog` is installed in the `ai-sw-engineer` container or on the host, and
  the only prebuilt `obj_dir/` under ai_hw_engineer is for the 8273, not the
  4004, and there is no 4004 ISS or Python model. So chips is currently the only
  *executed* oracle; the RTL comparison is deferred until a simulator is wired
  into the container (tracked in `plans/testplan.md`). chips itself is validated
  upstream against MAME on the Busicom ROMs (per `isa.md` §10), so it is a
  strong single oracle, but it is one oracle, not two.

### Findings from execution (chips vs the spec)

1. **0xFF makes chips panic.** `spec/isa.md` §8 says the two undefined opcodes
   `0xFE` and `0xFF` "execute as NOP". Executed on chips: `0xFE` is a true NOP
   (its decoder has an explicit "does nothing" arm), but **`0xFF` hits an
   `unreachable!("Unknown opcode: F{:X}")` and aborts** — chips' accumulator-
   group match has no arm for modifier `0xF`. Our disassembler refuses both
   (safe). Resolution: this is a chips implementation gap, not a defect in our
   backend or a reason to change `isa.md` (the manual's "executes as NOP" is a
   documented reconstruction; chips simply never implemented 0xFF). Recorded;
   our conservative refusal is correct either way.
2. **JCN TEST-pin polarity.** For `JCN` condition `0x1` (TEST bit only, no
   invert), chips jumps when the TEST pin is **HIGH** and falls through when it
   is LOW. `isa.md` §4 describes the TEST condition as "TEST pin = 0". This is a
   chips-vs-manual semantic disagreement on the TEST line's active level. It
   does **not** affect our disassembler, which only prints the 4-bit condition
   field (verified correct for all 16 conditions) and does not interpret it;
   resolving the polarity needs the RTL as the tie-breaker, so it is left open
   and noted here rather than silently resolved.

The harness recovers hidden CPU state by appending readout instructions; it is
run twice with two different scratch-pointer register pairs so that a register
used as the readout pointer in one pass is read cleanly in the other (an
earlier single-pass version mis-reported R14/R15 as the pointer's char index —
a harness bug, caught and fixed before any result was reported).

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
- **RTL half of the cross-check not executed**: the milestone-3a differential
  check (see "Differential cross-check") executes every encoding on
  `external/chips`, but the second oracle — the `ai_hw_engineer`
  `design/intel_4004` RTL — is **not run**, because no Verilog simulator
  (`verilator` / `iverilog`) is present in the container or on the host and
  there is no 4004 ISS. chips is validated upstream against MAME, so it is a
  strong single oracle, but chips-vs-RTL is still owed. Tracked in
  `plans/testplan.md`. (Note: this supersedes the milestone-2 note that no
  second oracle was available at all — `external/chips` is now checked out and
  executed.)
- **ELFDumper / Triple / ELF infra edits** are applied by the overlay script,
  not committed into the submodule. They are small and idempotent; if the
  pinned tag ever moves, re-verify the anchors in `scripts/apply_overlay.py`.

## Spec corrections / findings

- No `isa.md` / `abi.md` corrections were needed: the opcode table and
  relocation list were complete and the implemented encodings matched them
  exactly on the first passing build.
- **Finding (chips vs spec, 0xFF):** executing the undefined opcodes on chips
  shows `0xFE` is a true NOP but **`0xFF` panics** (`unreachable!("Unknown
  opcode: FF")`) — chips never implemented the `0xF?` modifier `0xF` arm, so it
  does not match `isa.md` §8's "executes as NOP". Our disassembler refuses both,
  which is safe. No backend change.
- **Finding (chips vs spec, JCN TEST polarity):** chips jumps on `JCN 0x1` when
  the TEST pin is HIGH; `isa.md` §4 phrases the TEST condition as "TEST pin =
  0". A semantic disagreement on the TEST line's active level, needing the RTL
  as tie-breaker. Does not affect the disassembler (condition field printed
  correctly for all 16 conditions).
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
