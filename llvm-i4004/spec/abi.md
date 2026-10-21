# Intel 4004 ABI

Written before any backend code. `rust/i4004-none.json`'s `data-layout` must
equal the string in §1 **character for character** (CI greps both; see
`plans/testplan.md`). The target is a hostile fit for a C-like IR — a 4-bit
accumulator, sixteen 4-bit index registers, a 3-level hardware return stack,
and no general data-memory addressing except an `SRC` pointer into 4002
characters — so this ABI makes deliberate, documented trades rather than
pretending the 4004 is a tiny byte machine. The yardstick consumer is
`system/busicom_141pf/firmware`: `no_std`, no integer wider than `u8`,
digit-serial BCD in `u8` carries (`firmware/src/bcd.rs`), a 1280-byte original
ROM budget (4096-byte hardware max), and ≤ 3-deep call graph by design.

The 6502 precedent is **llvm-mos** (`external/llvm-mos`, read-only here): it
keeps `i8` as the native unit, places LLVM "imaginary" registers in zero-page
RAM, and passes arguments in those memory-backed registers. The 4004 has no
zero page and no 8-bit ALU, so we adapt that idea onto 4002 RAM rather than
copy it.

## 1. Data layout

**Chosen `data-layout` string (must match the JSON exactly):**

```
e-m:e-p:16:8-p1:16:8-p2:16:8-i8:8-i16:8-n8-a:8
```

Field-by-field justification:

| Field | Meaning | Why |
|---|---|---|
| `e` | little-endian | §1.1 |
| `m:e` | ELF name mangling | object format is ELF (§5) |
| `p:16:8` | default (data) pointer: 16-bit, 8-bit ABI alignment | §1.3 — a flat 16-bit handle encodes the 4002 character address; alignment 8 because the minimum addressable unit is a byte |
| `p1:16:8` | addrspace 1 (ROM/code constants) pointer: 16-bit | §1.3 — 12-bit ROM address zero-extended into 16 bits |
| `p2:16:8` | addrspace 2 (RAM status characters) pointer: 16-bit | §1.3 |
| `i8:8` | i8 aligns to 8 bits | the addressable unit |
| `i16:8` | i16 aligns to 8 bits (not 16) | no alignment benefit on a byte-serial machine; keep everything byte-aligned to avoid padding in the tiny RAM |
| `n8` | native integer width = 8 | §1.2 — see the i4-vs-i8 decision |
| `a:8` | aggregate (struct) alignment 8 bits | pack structs to the byte; no wider natural alignment exists |

### 1.1 Endianness

**Little-endian.** The hardware is nibble-serial and the historical bus sends
the low nibble first (PC low nibble at A1; `FIM`/`SRC` treat the pair's even
register as the high nibble but memory-order is a software choice). The
Busicom BCD core stores least-significant digit first (`bcd.rs`: "`d[0]` is the
least significant digit"), so little-endian matches both the natural
multi-nibble arithmetic direction and the existing portable code. LLVM also
handles little-endian legalization most smoothly.

### 1.2 Native integer width: `n8`, not `n4:8` — the central trade

LLVM requires that the **minimum addressable unit be at least 8 bits**: a
`store i8` must address a whole byte, and LLVM has no concept of a 4-bit memory
cell. We therefore cannot make `i4` the addressable unit. The options:

1. **`n4:8` (native i4 and i8), byte = 8 bits = two 4002 nibbles.** Models the
   accumulator and index registers as `i4` natives and composes a byte from two
   adjacent 4002 characters. *Rejected as the data-layout native set* because
   declaring `i4` native tells LLVM's legalizer that 4-bit loads/stores and
   4-bit arithmetic are directly supported in *memory*, which is false — memory
   is byte (two-nibble) addressed, and a stray `i4` in the layout string makes
   the type legalizer and struct layout reason about 4-bit storage units that
   do not exist. It also fights `getMinimumJumpTableEntries` and alignment
   math.
2. **`n8`, byte = 8 bits stored as two 4002 nibbles (chosen).** The addressable
   unit and the only *declared* native width is `i8`. `i4` remains a perfectly
   legal **value** type the backend lowers specially (it is the accumulator's
   and an index register's natural width), but it is **not** in `n...` and is
   **not** an addressable memory unit. A `u8`/`i8` in RAM occupies two
   consecutive 4002 main characters, low nibble first (little-endian); 4-bit
   values live in index registers or are the low nibble of a byte cell.

The decisive reason: the yardstick firmware does all arithmetic in `u8`
(`bcd.rs` carries, `keys.rs`, `print.rs`) and never needs a 16/32-bit integer
for the calculator. Declaring `n8` makes `i8` the "goal width" the legalizer
promotes to, which is exactly the width the real code uses, and lets the
backend special-case `i4` in instruction selection (one index register, or the
`ACC`) without lying about memory. Wider types (`i16`, `i32`) are legal IR but
are **expanded** to byte operations by the backend's custom legalization and
runtime helpers (§7); they are deliberately absent from `n...` so the legalizer
splits them.

`ASSUMPTION:` i4 as a non-addressable native value width is a backend
convention, not expressible in the data-layout string; recorded here and to be
re-stated as an `ASSUMPTION:` comment in the TableGen register/type setup.

### 1.3 Pointers and address spaces

MCS-4 has **separate, non-overlapping** program (ROM) and data (RAM) spaces
plus RAM status characters, so a single flat pointer cannot name every datum.
We use LLVM address spaces:

| AS | Name | Points at | Pointer encoding (16-bit value) |
|---|---|---|---|
| 0 | **data** (default) | 4002 RAM **main** characters | `[bank:2][chip:2][reg:2][char:4]` in bits, zero-extended to 16; byte cells occupy two adjacent chars |
| 1 | **code / ROM** | 4001 ROM bytes and `.rodata` constants (read via `FIN`/`WRR`-free loads) | 12-bit ROM address `{chip:4, word:8}` zero-extended to 16 |
| 2 | **status** | 4002 RAM **status** characters (4 per register, opcode-addressed) | `[bank:2][chip:2][reg:2][statnum:2]`, zero-extended to 16 |

- **Pointer width = 16 bits** for all three. The address-generating fields fit
  in 10–12 bits; 16 is the smallest power-of-two ≥ 12 that LLVM handles
  cleanly, and it leaves room for the AS-tagged encoding. `target-pointer-width`
  in the JSON is therefore **16** (§ rust/i4004-none.json), not matching any
  single hardware register — a pointer is a *software* handle the backend
  decomposes into an `SRC` pair + bank select, not a machine address register.
- **ROM constants** (string tables, the printer drum map in `print.rs`) live in
  addrspace 1 and are reached with `FIN` (pair 0 indexed fetch) within a page,
  or via a generated trampoline for cross-page constant tables (§7). Rust
  `&'static` data lowers to addrspace-1 pointers.
- A frontend emitting a plain `*const u8` gets addrspace 0 (RAM). Taking the
  address of a `static` constant yields addrspace 1. `addrspacecast` between 0
  and 1 is **illegal** (the spaces are physically distinct) and the backend
  errors on it, matching the hardware.

`ASSUMPTION:` the exact bit packing of the AS-0/AS-2 pointer is a backend
convention (the SRC protocol only needs chip/reg/char); recorded here, to be
pinned when the pointer-lowering pass is written and tested.

## 2. Registers and register classes

| Class | Registers | Width | Allocatable | Role |
|---|---|---|---|---|
| `ACC` | `ACC` | 4 | no (fixed) | accumulator; implicit operand/result of the whole accumulator group and ADD/SUB/LD/XCH |
| `CARRY` | `CY` | 1 | no (fixed) | carry/borrow + the extra bit for multi-nibble chains and `JCN` |
| `GR4` | `R0`–`R15` | 4 | **yes**, except reserved below | general 4-bit index registers |
| `PR8` | `P0`–`P7` (pairs R0:R1 … R14:R15) | 8 | **yes**, except P0 | 8-bit values and pointers; the natural `i8` register class |

Reserved / special index registers (not allocatable to the general pool):

- **P0 (R0:R1)** is reserved: it is the hardware index for `FIN`/`JIN` (indirect
  ROM fetch and indirect jump). The backend uses it as the **scratch pointer**
  for `FIN`-based constant loads and indirect calls; it is caller-clobbered and
  never holds a live value across those operations.
- **R14:R15 (P7)** is reserved as the **software stack pointer (SSP)** low/high
  nibbles when the software return stack is in use (§4.4). On a leaf-heavy
  program that never overflows the 3-level hardware stack, P7 is freed back to
  the allocator (a module-level attribute decides; default: reserved so stack
  spills always have a pointer).

So the **general allocatable pool** is R2–R13 (twelve 4-bit registers = six
8-bit pairs P1–P6), with P0 and P7 reserved as above. `ACC`/`CY` are fixed
hardware resources the scheduler models as implicit defs/uses, not allocatable
GPRs.

**CodeGen register classes (implementation, matching this ABI).** `i8` is the
legal integer in class `PR8` (the pairs P0–P7). `i16` and pointers are legal in
class `DR16`, a **double-pair** D0..D3 where `Dk = {P(2k):P(2k+1)}` — four
nibbles / 16 bits (this mirrors AVR's `GPR8`/`DREGS`: a 16-bit value is legal in
a register pair-of-pairs, but 16-bit arithmetic is expanded to i8 ops on the two
sub-pairs). D0 contains the reserved P0 and D3 contains the reserved P7, so the
allocatable i16 pool is effectively D1 (P2:P3) and D2 (P4:P5). A pointer's low
sub-pair holds the `SRC` address byte `[chip:2][reg:2][char:4]` (§1.3); the high
sub-pair holds the bank/extended bits. This is the "software handle the backend
decomposes into an SRC pair + bank select" named above, made concrete — no ABI
value changes, so `rust/i4004-none.json` is unaffected.

### 2.1 Imaginary (memory-backed) registers

Like llvm-mos's zero-page imaginary registers, the 4004 backend defines a bank
of **memory-backed pseudo-registers in 4002 RAM** for when twelve index
registers are not enough (spills, wide-type lowering, the soft stack frame):

- A fixed region of **4002 bank 0, chip 0, register 0** — 16 main characters =
  8 byte-cells — is the **imaginary register file** `IR0`–`IR7` (`i8` each),
  addressed by a constant `SRC` pointer. These are **caller-saved scratch** by
  default.
- The backend prefers hardware index registers; it demotes a virtual register
  to an imaginary register only on spill. Each imaginary-register access costs
  an `SRC` (once, hoistable if the pointer is unchanged) plus `RDM`/`WRM`.
- The firmware's own working registers (`bcd::Num`, 16 nibbles each) map
  naturally onto 4002 registers; the ABI reserves chip0/reg0 for the compiler
  and leaves the rest to the program's statically-placed data (the linker
  script assigns them, §5 / `platform/memory_map.md`).

Source for the pattern: llvm-mos imaginary/zero-page register convention
(`external/llvm-mos`), adapted to 4002 characters because the 4004 has no
zero-page and no 8-bit load/store — every imaginary-register touch is an
`SRC`+`RDM`/`WRM` pair, not a single addressed instruction.

## 3. Fundamental types

| Type | Size | Alignment | Representation |
|---|---|---|---|
| `bool`/`i1` | 1 byte cell | 8 | 0 or 1 in the low bit of a byte cell |
| `i4` (value only) | — | — | one index register or the low nibble of a byte cell; never an addressable memory unit (§1.2) |
| `i8`/`u8`/`char`-nibble | 1 byte = 2 nibbles | 8 | two adjacent 4002 chars, low nibble first |
| `i16` | 2 bytes | 8 | expanded; little-endian byte pair |
| `i32`/`i64` | 4/8 bytes | 8 | expanded via runtime helpers (§7); discouraged — none in the firmware |
| pointer (AS 0/1/2) | 2 bytes | 8 | 16-bit handle (§1.3) |
| `enum`/aggregate | sum of fields | 8 | byte-packed, `a:8` |

**C `int`:** `target-c-int-width` = **16**. Rationale: the firmware is Rust and
uses `u8` throughout, but a 16-bit `int` is the smallest width that keeps C
`int` arithmetic from silently truncating `u8*u8` products and matches
`target-pointer-width`. A `char` is 8 bits. (If a future C consumer proves
16-bit `int` too expensive, an 8-bit `int` is possible but non-conforming to
C's `INT_MIN`/`INT_MAX` range requirement — recorded in Open Questions.)

## 4. Calling convention — "i4004-C"

Designed around the hardware's three hard constraints: a single 4-bit ACC, a
3-level return stack, and no cheap memory addressing. The guiding principle is
the one the firmware already follows — **keep the call graph shallow and pass
little** — so most calls need no software stack at all.

### 4.1 Argument passing

Arguments are classified left to right and assigned to **register pairs**, then
to the imaginary-register file, then to a RAM argument area:

1. The first **three** integer/pointer arguments that fit in 8 bits go in pairs
   **P1, P2, P3** (an `i4`/`bool` occupies the low nibble of a pair, high nibble
   zeroed; an `i8` fills the pair; a 16-bit pointer takes **two** pairs).
2. Further scalar arguments, and the high half of a split 16-bit value when
   P-pairs run out, go in **imaginary registers IR0…** in order.
3. Aggregates larger than 2 bytes, and anything past the imaginary file, are
   passed **by reference**: the caller materializes the value in a RAM
   temporary and passes an addrspace-0 pointer in the next pair/imaginary slot.

Pair P0 is never an argument register (reserved, §2). This mirrors llvm-mos
passing in a small set of memory-backed registers, scaled down to the 4004's
tiny register budget.

### 4.2 Return values

- `i4`/`bool`: **ACC** (low nibble), with `CY` undefined on return unless the
  function's signature declares a carry result.
- `i8`: **P1** (R2:R3).
- 16-bit pointer / `i16`: **P1:P2**.
- Larger: **sret** — the caller passes a hidden addrspace-0 pointer as the
  first argument (in P1), the callee writes through it, and returns that
  pointer in P1.

### 4.3 Caller/callee-saved

- **Caller-saved (clobbered):** `ACC`, `CY`, `P0` (scratch pointer), `P1`–`P5`
  (argument/return), and all imaginary registers `IR0`–`IR7`. `P1`–`P3` carry
  i8 arguments/returns; `P2:P3` (**D1**) and `P4:P5` (**D2**) are the two
  i16/pointer argument-and-return double-pairs (§4.1). A Rust multi-field
  return such as `{ i16, ptr }` places its first word in D1 and its second in
  D2, so **D2 cannot also be callee-saved** — a call defines (clobbers) P4:P5.
- **Callee-saved (preserved):** `P6` (R12:R13) only. A callee that uses it
  saves/restores via the software stack or imaginary file. (Earlier drafts kept
  P4,P5,P6 callee-saved; that was incompatible with returning a two-word
  aggregate in D2 — LiveIntervals rejected the `COPY $d2` reading the second
  return slot because the call never defined it. Making D2 caller-saved resolves
  it and lets the two i16 arg/return pairs D1,D2 both be freely clobbered.)
- `P7` (SSP) is **reserved**, neither caller- nor callee-saved — it is the
  stack machinery itself (§4.4).

### 4.4 Stack discipline and the hardware-stack limit

The 4004 return stack is **3 levels** (4 slots, one being the live PC — §6 of
`isa.md`). The ABI defines a two-tier scheme:

1. **Hardware return stack (default, depth ≤ 3).** `JMS`/`BBL` are used
   directly. The backend tracks static call depth; when the whole reachable
   call graph from an entry point is provably ≤ 3 deep (the Busicom firmware is
   written this way — `firmware/plans/implementation_plan.md`: "the 3-level
   4004 call stack means the backend will inline or use a RAM software stack;
   keep the core's call graph shallow"), **no software stack is emitted at
   all**. Leaf functions and shallow trees are the common case.
2. **Software return stack (depth > 3).** When static depth can exceed 3, the
   backend switches a subtree to a **software-managed return stack in 4002
   RAM**, pointed at by **P7 (SSP)**. The convention:
   - The soft stack lives in a dedicated 4002 region assigned by the linker
     script (`platform/link.ld` / `memory_map.md`); it **grows upward** (SSP
     increments) because 4002 character addresses increment and `ISZ`-style
     increment is cheaper than decrement.
   - A call that must use it does: push the 12-bit return address as **three
     nibbles** (`WRM` ×3 through SSP, SSP += 3 via `INC`/`ISZ`), then `JUN` to
     the callee (not `JMS` — the hardware stack is bypassed). Return pops three
     nibbles (`RDM` ×3), reconstructs the address into pair 0, and `JIN`s to it.
   - **Frame layout** (per software-stack call), low address first:
     ```
     SSP-3 : return addr nibble 0 (offset low)      [char n]
     SSP-2 : return addr nibble 1 (offset high)      [char n+1]
     SSP-1 : return addr nibble 2 (page)             [char n+2]
     ```
     Spilled callee-saved pairs and large locals, if any, are placed by the
     backend *below* the return-address triple in the same 4002 region, each as
     byte-cells (two nibbles). There is no frame pointer; offsets are computed
     from SSP by constant `INC`.
   - **Reentrancy / interrupts:** none — the 4004 has no interrupts, so the soft
     stack needs no critical sections.
3. **Inlining first.** Because every software-stack call costs ~6 memory ops
   each way, the backend inlines aggressively at `opt-level=z` within the ROM
   budget; the soft stack is the fallback for genuine deep/recursive graphs,
   which the firmware does not have.

`ASSUMPTION:` the soft-stack RAM region and its size are a linker-script
decision (`platform/link.ld` is still TODO); the ABI fixes only the frame
*layout* and the SSP register, not the base address.

### 4.4.1 Concrete convention (PROVEN on chips, 2026-10-19)

The whole-firmware call depth (`run→press→dispatch→add_sub→add_mag→reg_add…`,
6+) exceeds the 3-level hardware stack, so compiled calls MUST use the soft
stack. The following sequences are validated on `external/chips`
(`rt/tests/softcall_core.s` depth-4 single-page; `rt/tests/softcall_mp.s`
cross-page) and are the contract the backend + packer implement.

**Return token** — 3 nibbles pushed low-address-first, SSP (P7) grows up:
```
[SSP+0] = return offset low nibble
[SSP+1] = return offset high nibble
[SSP+2] = return page nibble            (+ bank nibble folded here when banked)
```

**Compiled call** (`CALLSOFT`, expanded post-RA at the call site):
```
    fim  p1, <lo8 of .Lret>     ; R2:R3 = return offset (page-local low byte)
    fim  <reg>, <page of .Lret> ; return page nibble
    jms  __push_ret             ; leaf helper: pushes R3,R2,<page>; SSP+=3; bbl
    jun  <callee>               ; NOT jms — hardware stack bypassed
.Lret:
```
`__push_ret` is a single-level leaf (JMS/BBL, hardware depth 1).

**Compiled return** — the function's epilogue is `jun __soft_ret` instead of
`bbl`. `__soft_ret` (common, one copy) pops the token and dispatches:
```
    ; pop page -> Rp ; pop off-hi -> P0.hi ; pop off-lo -> P0.lo  (SSP-=3)
    ; P2 = &__srtab + page*2   (two-nibble add; __srtab is page-local to __soft_ret)
    jin  p2                     ; -> __srtab[page], a `jun <page_stub>`
__srtab: jun stub_p0 ; jun stub_p1 ; ... one JUN per ROM page
```
Each ROM page carries a **one-byte `jin p0` stub at a FIXED reserved offset**
(proof uses 0xF0). `__srtab[page]` JUNs to that page's stub; `jin p0` sets
PC[7:0]=P0 (the return offset) keeping the page → lands exactly at `.Lret`.

**Return values** stay in P1/D1; callee-saved pairs unchanged; `__push_ret`/
`__soft_ret` touch only ACC, P7(SSP), and scratch P0/P2 + one page reg (all
caller-clobberable at a call boundary).

### 4.4.2 Packer / backend contract

- **Packer reserves** the `jin p0` stub byte at the fixed offset on EVERY ROM
  page (common region + every bank), emits the stub, builds `__srtab` (16 JUNs)
  in `__soft_ret`'s page, and resolves each `jun stub_pK` to the reserved offset
  on page K. With banking, the page reg also drives the bank latch write in
  `__soft_ret` before dispatch (bank nibble from the token).
- **Backend** lowers `i4004_call` to `CALLSOFT` (above) and every non-leaf
  function's return to `jun __soft_ret`. `JMS`/`BBL` remain ONLY for leaf
  libi4004rt/HAL helpers whose own call graph keeps total hardware depth ≤ 3.
- **Packer asserts** the leaf rule: any function reached by `JMS` must itself
  contain no `JMS` to a non-leaf (i.e. the JMS-reachable subgraph is ≤ 3 deep).

## 5. Relocations and object format

**Object format: ELF**, little-endian, with a **private `e_machine`**. No
official EM_* value exists for the 4004; we use a provisional value in the
`EM_*` "unofficial/experimental" range and record it here so the assembler,
`ld.lld`, and any image packer agree:

- `e_machine = 0x4004` (decimal 16388) — provisional, documented, and gated
  behind a constant in both the MC backend and the linker. (If upstream LLVM
  ever assigns a real number, this is the single place to change.)
- `ELFCLASS32`, `ELFDATA2LSB`, `ET_EXEC` for a linked ROM image.

### 5.1 Relocation kinds

Named `R_I4004_*`; each fixes up a field of one or two program bytes:

| Reloc | Fixes | Width | Range / constraint |
|---|---|---|---|
| `R_I4004_ADDR12` | `JUN`/`JMS` second word + low nibble of word 1 | 12 bits across 1.5 bytes | any ROM byte 0x000–0xFFF |
| `R_I4004_PAGE8` | `JCN`/`ISZ` second word (`A2A1`) | 8 bits | target must share the page the PC holds after both words are fetched; **page-crossing is an error** unless the end-of-page roll-forward is explicit (§9 of `isa.md`) |
| `R_I4004_IMM8` | `FIM` second word | 8 bits | immediate; no range limit |
| `R_I4004_FIN8` | pair-0 index for a `FIN` constant load | 8 bits | the constant must lie on the **current page**; the linker verifies page identity |
| `R_I4004_ROMADDR12` | an addrspace-1 pointer stored as data | 12 bits (zero-extended in a 2-byte cell) | ROM byte |
| `R_I4004_RAMADDR16` | an addrspace-0/2 pointer stored as data | 16-bit handle (§1.3) | RAM char |

**Page-crossing constraint (the hard one):** `R_I4004_PAGE8` and `R_I4004_FIN8`
are **page-local**. The linker must guarantee the referent is reachable without
leaving the 256-byte 4001 page; if placement would cross a page it must either
(a) relocate the target into the page, or (b) emit a `JUN` trampoline on-page
and retarget the `JCN`/`ISZ` to it. The image packer in `asm/` owns 256-byte
page layout for exactly this reason (`asm/README.md`).

### 5.2 Linking / image packaging

Preferred: LLVM MC + `ld.lld` with an i4004 target hook. The linker script
(`system/busicom_141pf/platform/link.ld`, currently TODO) places `.reset` at
ROM 0x000, `.text`/`.rodata` into the 1280-byte (max 4096-byte) ROM, and the
soft stack + imaginary file + statics into 4002 RAM per `memory_map.md`. A
small Rust image packer in `asm/` is the fallback if `ld.lld`'s page-aware
placement proves insufficient; `scripts/build_image.sh` is its contract.

## 6. Runtime library (`libi4004rt`)

LLVM will lower operations the ISA cannot do in one instruction to calls into a
`no_std` Rust/assembly runtime. Helpers live in `asm/rt/` (or a `compiler_rt`
shim), are themselves written to the shallow-call-graph rule, and are
size-optimized because they count against the ROM budget.

| Helper (symbol) | Lowered from | Notes |
|---|---|---|
| `__mulqi3` / `__mulhi3` | `i8`/`i16` multiply | shift-and-add; `i8*i8→i8` is the common one (BCD digit scaling) |
| `__udivqi3` / `__umodqi3` | `i8` unsigned div/mod | restoring division; no HW divide |
| `__divqi3` / `__modqi3` | signed div/mod | sign-handle then call unsigned |
| `__ashlqi3` / `__lshrqi3` / `__ashrqi3` | variable shifts | `RAL`/`RAR` loops counted by an index register |
| `__ashlhi3` … | 16-bit variable shifts | nibble/byte reshuffle + bit loop |
| `memcpy` / `memset` / `memmove` | `llvm.memcpy`/`memset` intrinsics | `SRC`+`RDM`/`WRM` loops over 4002 chars; `memmove` picks direction by overlap |
| `__cmphi` / comparisons | `i16` compares | byte-wise from the top |
| **BCD helpers** | — | thin wrappers exposing `DAA`, `KBP`, `TCS`, `DAC` so the firmware's digit-serial code (`bcd.rs`) maps to single instructions instead of synthesized arithmetic: `__bcd_adjust` (DAA), `__key_decode` (KBP one-hot→binary), `__tens_comp` (TCS ten's-complement step), `__dec` (DAC) |

Design note: the BCD helpers exist specifically because the yardstick firmware
is a BCD calculator. `DAA`/`KBP`/`TCS` are single 4004 instructions; exposing
them as intrinsics/builtins (rather than letting LLVM synthesize decimal
adjust out of binary ops) is the difference between fitting 1280 bytes and not.
The backend should pattern-match the firmware's `add_mag`/`sub_mag` nibble
loops (`bcd.rs`) onto `ADD`/`DAA` and `SUB`/`CY` sequences directly where it
can, and fall back to these helpers otherwise.

`16`/`32`-bit integer helpers are provided for completeness but are **not
exercised by the first consumer** — the firmware uses only `u8`/nibble
arithmetic (`firmware/README.md`), so these are untested until a title needs
them (Open Questions / testplan).

### 6.1 Implemented runtime (`llvm-i4004/rt/`, branch `llvm/i4004-runtime`)

The runtime now exists in 4004 assembly and is **execution-verified on
`external/chips`** against a host reference (624 arithmetic/shift/logic cases +
HAL/mem/stack suites, all green). Implemented: `__mulqi3`, `__udivqi3`,
`__umodqi3` (+ internal `__udivmod`), `__negqi2`, `__ashlqi3`/`__lshrqi3`/
`__ashrqi3`, `__andqi3`/`__orqi3`/`__xorqi3` (bit-serial, no ROM table),
`__memset_ram`/`__memcpy_ram`/`__memmove_ram` (over 4002 **nibbles**), the BCD
wrappers `__bcd_adjust`/`__key_decode`/`__tens_comp`/`__dec`, and the
software-stack helpers `__push_byte`/`__pop_byte`/`__push3`/`__pop3`. Signed
`__divqi3`/`__modqi3` are deliberately deferred (the firmware never needs them;
shipping unverified would break "prove it"). Full register contracts and sizes:
`llvm-i4004/rt/README.md`. Library + crt0 + HAL = 697 bytes of the 1280 budget.

**Two ABI points the backend's codegen must honour (both load-bearing,
discovered while verifying the runtime on `chips`):**

1. **Multi-nibble subtract needs `CMC` between nibbles.** `SUB`/`SBM` compute
   `ACC + ~R + ~CY` (isa.md §7). A multi-nibble (`i8`/`i16`) subtract must do
   `CLC` once, `SUB` the low nibble, then **`CMC` before each higher nibble's
   `SUB`**; otherwise the borrow propagates with the wrong sense and every
   nibble above the lowest is off by one. The final `CY = 1` means "no borrow"
   (`a >= b`). Verified on `chips`.
2. **`BBL` clobbers the `i4` ACC return.** `BBL DDDD` loads `ACC ← DDDD` on
   return, so a `JMS`/`BBL` callee **cannot** return an `i4` in ACC — the i4/i8
   return register for an out-of-line call is **P1** (low nibble for i4). §4.2's
   "ACC for i4" applies only to inlined / `JUN`-tail code. The backend should
   prefer inlining the single-instruction BCD ops (keeping the result in ACC)
   over calling the wrappers.

## 7. Open questions

These are explicit unknowns, **not** guesses baked into the spec:

1. **SUB inversion discrepancy.** The MCS-4 manual's printed `SUB` equation
   omits the `~` that `SBM` shows; this ABI/ISA follows the inverted
   (no-borrow-is-CY=1) sense per the ai_hw_engineer spec and MAME. If a title's
   oracle ever disagrees, this is the first suspect. (Tracked in `isa.md` §7.)
2. **Exact AS-0/AS-2 pointer bit-packing** (§1.3) is a backend convention not
   yet pinned by a test; fix it when the pointer-lowering pass lands.
3. **`i4` as a non-addressable native value width** (§1.2) — confirm the
   legalizer and TableGen type setup accept `i4` as a legal register value while
   `n8` keeps it out of memory-unit reasoning; may need a custom `ValueType`.
4. **Soft-stack base address and size** (§4.4) depend on `platform/link.ld`,
   still TODO; the firmware's RAM budget (`memory_map.md`: 160 nibbles) leaves
   little room, so deep call graphs may be infeasible and inlining mandatory.
5. ~~Provisional `e_machine`~~ **Decided 2026-10-18 (project owner): `e_machine
   = 0x4004`** (§5) is the value for this toolchain. It is private and
   unregistered; the remaining check is that `ld.lld` does not special-case it,
   and a migration note is needed only if upstream ever assigns a real EM_*.
6. **16/32-bit runtime helpers** (§6) are unverified until a non-firmware title
   needs them; their calling-convention cost may force a different split.
7. **Decided 2026-10-18 (project owner): C `int` = 16 bits** (`target-c-int-width` = 16),
   conforming C. The 8-bit alternative is rejected.

## 8. Sources

- Chip behavior: `isa.md` (this backend) and ai_hw_engineer
  `design/intel_4004/spec/spec.md`.
- Consumer constraints: `system/busicom_141pf/firmware/` (`README.md`,
  `src/bcd.rs`, `plans/implementation_plan.md`) and
  `system/busicom_141pf/platform/memory_map.md`.
- 6502 ABI precedent (imaginary/zero-page registers, memory-backed argument
  passing): llvm-mos, `external/llvm-mos` (read-only); project background in
  `README.md` "Strategy".
- MCS-4 manual provenance: `reference/README.md` and `isa.md` §10.
