# llvm-i4004 test plan

| Suite | Tool | Oracle | Location |
|---|---|---|---|
| MC encode | `llvm-mc -show-encoding` + FileCheck | manufacturer opcode table (`spec/isa.md`) | `tests/mc/` |
| MC errors | `llvm-mc` + `not` + FileCheck | spec: page-local / range rules | `tests/mc/` |
| Codegen | `llc` + FileCheck | hand-checked expected asm | `tests/codegen/` |
| Execution | simulator | expected register/memory state | `tests/smoke/` |
| Rust end-to-end | `rustc` → image → simulator | known output | `tests/smoke/` |
| ABI/JSON consistency | `python -m json.tool` + `grep` | abi.md data-layout string | CI script |

Execution tests run on the ai_hw_engineer RTL/ISS where one exists and on
`external/chips` otherwise. When the two disagree, that is a finding
(AGENTS.md), logged in `failure_notes/`.

## Milestone-1 validation (specs) — runnable now

1. **Data-layout identity.** The exact string
   `e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8` must appear in both
   `spec/abi.md` §1 and `rust/i4004-none.json`:
   `grep -F 'e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8' spec/abi.md rust/i4004-none.json`
   must match both files.
2. **JSON parses.** `python -m json.tool rust/i4004-none.json` (or the
   `ai-sw-engineer:latest` docker image) exits 0.
3. **All 46 opcodes present.** Every mnemonic in `isa.md` §1–§3 has an encoding
   row; the count of defined instructions is 46.
4. **Markdown links resolve.** Every relative link in the backend READMEs/specs
   points at a file that exists.

## Milestone-2 status -- DONE (10/10 MC tests pass)

Implemented in llvm-i4004/tests/mc/ and run by scripts/run_tests.sh against
the LLVM 23.1.3 build in llvm-i4004/install/bin:

- basic.s (16 OPR=0x0..0xD), ram_rom.s (16 OPR=0xE), accgroup.s (14 OPR=0xF),
  jcn_cond.s (JCN conditions + jmp alias), pairs.s (all 8 pair forms),
  reloc.s (symbolic two-word fixups), elf.s (e_machine 0x4004 + R_I4004_* via
  llvm-readobj).
- errors/: err_imm_range.s, err_pair_vs_reg.s, err_undefined.s.
- All 46 opcodes assemble to the isa.md byte column; see report/final_report.md.

**Deferred to milestone 5 (linker/image packer):** err_page_cross.s. The
R_I4004_PAGE8 / R_I4004_FIN8 relocations are emitted, but the end-of-page
(isa.md sec.9) *rejection* needs the linker-side page-identity check, which
does not exist yet. R_I4004_FIN8 is defined but not yet exercised by a test
(no FIN constant-table assembly path until codegen needs it).

## Milestone-2 MC encode tests (next, concrete)

The `tests/mc/` fixture is generated directly from the `isa.md` tables — one
`CHECK` per opcode, so the opcode table *is* the oracle:

- `tests/mc/basic.s` — the 16 `OPR=0x0..0xD` rows, e.g.
  `# CHECK: ldm 9  ; encoding: [0xd9]`, `# CHECK: jun start ; encoding: [0x4A,0xA1]`
  (addr fixup), `# CHECK: fim p1, 0x3c ; encoding: [0x22,0x3c]`.
- `tests/mc/ram_rom.s` — the 16 `OPR=0xE` rows (`wrm`=0xE0 … `rd3`=0xEF).
- `tests/mc/accgroup.s` — the 14 defined `OPR=0xF` rows (`clb`=0xF0 …
  `dcl`=0xFD); assert `0xFE`/`0xFF` are rejected or `.byte`-only.
- `tests/mc/jcn_cond.s` — each JCN condition mnemonic (`Z`,`C`,`T`,`NZ`,…) and
  hex form encode to the right `000X` OPA nibble; unconditional `JMP`/`JCN 0x8`.
- One assertion per **two-word** instruction that the second word is emitted and
  carries the right `R_I4004_*` fixup.

## Milestone-3a status -- disassembler + differential cross-check DONE

Implemented: the disassembler (`tests/mc/disasm/`, 16/16 lit pass incl.
byte-identical round trip) and a DIFFERENTIAL cross-check that EXECUTES every
encoding on `external/chips` and verifies our `llvm-mc --disassemble` output
(mnemonic + operands) predicts the observed effect.

- Tool: `tools/chips_xcheck` (Rust; depends on `external/chips` by path, read &
  run only). Driver: `scripts/crosscheck_disasm.py`; wired into
  `scripts/run_tests.sh`.
- Oracle = chips EXECUTION (not a hand table). 336 data/ALU/IO assertions +
  operand-field sweeps (all registers, immediates, FIM pairs) + 17
  control-transfer landings (JUN/JMS/JCN matrix/ISZ/JIN). No disagreements.
- Mutation-tested: `MUTATE=swap_acc` and `MUTATE=reg_off1` both produce the
  expected findings, so the check can fail.
- Findings logged: 0xFF panics on chips (vs spec "NOP"); JCN TEST-pin polarity
  differs from `isa.md` §4. Both in `report/final_report.md`.

**Owed — RTL half of the cross-check.** AGENTS.md wants chips-vs-RTL
disagreements treated as findings. The `ai_hw_engineer` 4004 RTL
(`design/intel_4004/src/intel_4004.sv`) is present but **cannot be run here**:
no `verilator`/`iverilog` in the `ai-sw-engineer` container or on the host, and
no 4004 ISS / prebuilt sim. To close this: add a Verilog simulator to the
container image, build the RTL, and extend `tools/chips_xcheck` with a second
executor backend that drives the RTL testbench with the same encodings + states
and diffs chips vs RTL. Until then chips (MAME-validated upstream) is the only
executed oracle.

## Milestone-2/3 MC error + codegen lit tests (per pattern)

MC error tests (`llvm-mc` must fail with the spec diagnostic):

- `tests/mc/err_page_cross.s` — a `JCN`/`ISZ`/`FIN` whose target leaves the
  current 256-byte page is rejected (end-of-page rule, `isa.md` §9).
- `tests/mc/err_imm_range.s` — `LDM 16`, `FIM P0, 256` rejected.
- `tests/mc/err_pair_vs_reg.s` — `SRC R3` (pair expected) / `ADD P2` rejected.

Codegen lit tests — **one FileCheck test per selection pattern** (`tests/codegen/`):

- `add_i4.ll`, `sub_i4.ll` → `ADD`/`SUB` with CY handling; `add_i8.ll` →
  two-nibble add with `DAA`-free binary carry via CY.
- `and_i4.ll` / `or` / `xor` → synthesized from `CMA`/`ADD` where no native
  logic op exists (document the sequence).
- `load_store_ram.ll` → addrspace-0 `load i8`/`store i8` lower to `SRC`+`RDM`/
  `WRM` (two chars, little-endian).
- `load_rom_const.ll` → addrspace-1 `load` lowers to a `FIN` on-page fetch.
- `br_incjcn.ll` → loop with `ISZ`; `br_cond.ll` → `JCN` on ACC=0 / CY.
- `call_leaf.ll` → `JMS`/`BBL`, no soft stack (depth ≤ 3).
- `call_deep.ll` → forced depth > 3 emits the P7 software-stack push/pop
  (`abi.md` §4.4) and matches the three-nibble frame layout.
- `ret_i8.ll` → return value in P1; `sret.ll` → hidden pointer in P1.

Runtime-helper tests (`tests/codegen/` + `tests/smoke/`):

- `mul_i8.ll` emits a call to `__mulqi3`; `udiv_i8.ll` → `__udivqi3`;
  `shl_var.ll` → `__ashlqi3`; `memcpy.ll` → `memcpy`.
- BCD: `daa.ll` lowers a decimal-adjust builtin to a single `DAA`; `kbp.ll` to
  `KBP`; verify against the firmware's `bcd.rs` nibble loops on the ISS.

## Known gaps / sequencing

- No TableGen/C++ yet (AGENTS.md: specs before code) — milestone 1 only.
- 16/32-bit helper tests are deferred until a non-firmware title needs them
  (the Busicom firmware is `u8`-only; `abi.md` §6/§7).
- Open questions in `abi.md` §7 gate milestone-2 design (esp. #3 i4-as-value).
