# llvm-i4004 code generation and linking design

How Rust source becomes a 4004 ROM image. `abi.md` is the contract (data
layout, registers, calling convention, relocations). This document explains
the machinery that implements it and why it is shaped that way. Status as of
2026-10-22 (issue #18). The Busicom firmware links flat at 4,095 B and runs
42/42 on the RTL and on external/chips.

## 1. Pipeline

```
Rust (no_std)
  └─ rustc --target avr-none, -Zbuild-std=core, fat LTO   one LLVM IR module
       └─ rust/retarget_ir.py: header rewritten to i4004
            └─ llc -mtriple=i4004 -function-sections        ELF objects
                 (+ hand-written .s through the same MC assembler)
                 └─ asm/i4004_pack.py                        flat ROM image + map
```

Driver scripts: `system/busicom_141pf/firmware/scripts/build_target_ir.sh`
(IR) and `scripts/build_image.sh` (llc + packer).

**Why AVR.** rustc has no 4004 target, and adding one means rebuilding rustc.
AVR's IR is the closest match: 8-bit integer ops and 16-bit pointers. The
mapping and the guards that refuse unsupported IR are in `rust/README.md`.

**Cost of that choice.** The mid-end (inlining, unrolling) runs with AVR's
cost model, not the 4004's. Issue #18 task C3 (a native target with an i4004
`TargetTransformInfo`) would fix it. It was not needed to fit, so it is not
done.

## 2. Register model: allocate pairs, execute nibbles

The 4004 computes through one 4-bit accumulator. If instruction selection
emitted nibble code directly, every value would pass through ACC, and the
register allocator would have nothing useful to allocate.

The backend therefore splits the work in two:

1. **Selection and register allocation work on 8-bit pseudos.**
   - `i8` is legal in `PR8` (pairs P0–P7). `i16` and pointers are legal in
     `DR16` (double pairs D1 = P2:P3, D2 = P4:P5). Wider types are legalized
     down to these.
   - ISel emits pseudo-instructions on pairs: `MOVi8`, `ADDi8`, `SUBi8`,
     `LOGICIi8`, `SHIFTIi8`, `NIBSELi8`, `BRCCi8`/`BRCCi8ri`, `RAMRS`/`RAMWS`,
     `LDRAMi8`/`STRAMi8`, `ZEXT8TO16`, `SPILL8/16`/`RELOAD8/16`, `CALL`, `RETpseudo`, …
   - Each pseudo declares `ACC`/`CY` as implicit defs. The greedy allocator
     sees a two-address machine with six usable pairs (P1–P6).
2. **`I4004ExpandPseudos.cpp` lowers each pseudo to real nibble code after
   register allocation.** For example, `ADDi8 Pd, Pa, Pb` becomes `CLC`, then
   `LD a.lo; ADD b.lo; XCH d.lo`, then `LD a.hi; ADD b.hi; XCH d.hi`.
   Subtract inserts `CMC` between nibbles (`abi.md` §6.1).
3. **`I4004Peephole` then drops a repeated `FIM`/`SRC` when the address pair
   still holds the same value.**

Most of the size work is choosing the cheapest expansion for common cases:

| Case | Expansion |
|---|---|
| compare with 0 | `LD r; JCN z/nz` per nibble (no subtract) |
| compare result type | `i8` (`getSetCCResultType`), not the pointer-sized `i16` |
| `zeroext`/`signext` `i1`/`i4` return | widened to `i8` in P1 (`getTypeForExtReturn`) |
| `and`/`or`/`xor` with a constant | nibble moves, `LDM`, `CMA`, or bit edits through CY |
| shift by a constant | nibble moves plus `RAL`/`RAR` (inline if ≤ 20 B) |
| `or` of operands with disjoint set bits | `add` (no carries possible) |
| `x * c`, c = ±(2^k ± 1)·2^m (i8: k, m ≤ 2; i16: k ≤ 1, m = 0) | shifts and one add/sub (`decomposeMulByConstant`) |
| variable shifts, wide mul/div | `libi4004rt` calls |

Every lowering has a lit test in `tests/codegen/`. The value-level lowerings
are also executed on chips: `scripts/exec_cmp.py` covers every compare
predicate, `scripts/exec_nibble.py` the nibble ops and constant multiplies.

## 3. Memory and frames

There is no data stack and no direct load/store. A data pointer names a 4002
character: `SRC` selects it, `RDM`/`WRM` (and `RD0-3`/`WR0-3` for status
characters) move one nibble. Address spaces separate RAM data, RAM status
characters and ROM constants (`FIN`); see `abi.md` §1.3.

**RAM intrinsics (issue #18, C4).** A call to `__ram_rd(a)`, `__ram_wr(a, v)`,
`__ram_rs<k>(a)` or `__ram_ws<k>(a, v)` is not a call. `LowerCall` turns it
into the pseudos `RAMRD`/`RAMWR`/`RAMRS`/`RAMWS` (and `RAMWRi`/`RAMWSi` for a constant value), which expand to `SRC` plus
one `RDM`/`WRM`/`RDk`/`WRk`. The status index `k` is part of the name, so it is
a constant. A title gets nibble-level RAM access with no pointer arithmetic
and no call overhead (the Busicom firmware's `src/ram.rs`).

**Static frames.** Each function that spills gets a static frame in RAM, not
a stack frame:
- The backend addresses every spill slot as `__frame_<fn> + offset` and
  emits the frame size as the absolute symbol `__framesize_<fn>`.
- The packer (`_alloc_frames`) builds the call graph from the `JMS`/`JUN`
  relocations. It then overlays frames: a function's frame starts after the
  frames of every possible caller, so frames that can be live at once never
  overlap, and siblings share space.
- Recursion is a link error.

The firmware itself keeps its numbers in 4002 registers through `Num`
handles. Frames only hold the compiler's spills.

**Banked frames (frozen).** `-i4004-banked-ram` wraps frame accesses in
`DCL` bank selects (`__framebank_<fn>`). This was part of the 2026-10-21
banked build and is not used by the flat image.

## 4. Calls

**Default: hardware calls.** Calls are `JMS` and returns are `BBL 0`. The
4004's return stack is 3 levels deep, so the program's static call depth
below `main` must be ≤ 3. The Busicom firmware is shaped for that:

| Level | Code |
|---|---|
| 0 | `main` (holds the run loop; entered by `JUN` from reset) |
| 1 | key handlers |
| 2 | assembly entries (`__n_*`), the printer, small Rust helpers that call only leaves |
| 3 | assembly leaves (`__leaf_*`, `__bus_*`) and local subroutines |

The packer's `_assert_call_depth` builds the `JMS` graph of the placed image.
It rejects recursion and fails, naming the deepest path, if the depth exceeds
3. A `JMS` inside the same section (an assembly routine's local subroutine)
counts as one level. The fix for an overflow is to inline a function, not to
turn soft calls back on.

**Leaf contract.** Calls to `__leaf_*` and `__bus_*` routines use a
preserved mask of P4–P6 (`CSR_I4004_Leaf`), not just P6. The compiler then
keeps values in P4:P5 across hot calls instead of spilling them. Hand-written
leaves must honour that (`abi.md` §4.3).

**Soft calls (frozen).** `-i4004-softcall` selects the 2026-10-19 software
return stack: `CALLSOFT`, `JUN __soft_ret`, P7 as the stack pointer, a
per-page `jin p0` stub. It is still built and tested but not used by the
flat image (`abi.md` §4.4.1).

## 5. MachineOutliner

The outliner runs after `I4004ExpandPseudos`, on real 4004 instructions
(`I4004InstrInfo.cpp`). It turns repeated sequences into shared functions:

- **Call form:** the sites `JMS OUTLINED_FUNCTION_n`; the body ends with
  `BBL 0`.
- **Tail form:** a sequence that ends with the function's own `BBL 0` is
  reached with `JUN` and keeps that `BBL`. This costs no stack level and no
  extra byte.

Constraints:
- **ACC liveness.** Real instructions do not list ACC as an operand, so its
  liveness comes from opcodes (`accUse`/`accLiveFrom`). `BBL 0` sets ACC to
  0, so a call-form site where ACC is live afterwards is dropped. A `JMS` to
  an outlined function counts as reading ACC, because the body may continue a
  computation in ACC.
- **What can't be outlined:** `JCN`/`ISZ` (page-local), `JUN`/`JMS` (would
  deepen the stack or move a target), `FIN`/`JIN` (depend on the page they
  run in), `DCL` (a bank bracket must stay whole), and pseudos.
- **Call depth:** an outlined call adds a call level. The packer's depth check
  covers it like any other `JMS`.
- **Where it runs:** off in the soft-call and banked-RAM layouts
  (`i4004LegacyLayout()`). Off in lit tests that check exact expansions
  (`-enable-machine-outliner=never`).

## 6. MC layer

- **Instructions:** TableGen definitions with an encoder and a disassembler.
  Every encoding is checked against the MCS-4 opcode table (`tests/mc`), and
  `scripts/crosscheck_disasm.py` executes every opcode on chips.
- **Objects:** ELF, `e_machine = 0x4004`.
- **Relocations** (`abi.md` §5.1): 12-bit absolute for `JUN`/`JMS`; 8-bit
  page-local for `JCN`/`ISZ` (`PAGE8`) and `FIN`; 8-bit immediate for `FIM`.
- **Hand-written assembly** (`rt/*.s`, a title's `asm/*.s`) goes through the
  same integrated assembler (`spec/asm_syntax.md`).

## 7. Linker: `asm/i4004_pack.py`

`ld.lld` has no notion of 4004 pages, so the project's own packer is the
linker. It reads the ELF objects and a linker script (ROM size, RAM layout),
and for a flat image it:

1. **Orders sections to fill pages** (`_page_packed`). `.reset` goes first and
   `.rodata` last. For each page it picks the subset of remaining sections
   that fills the rest of the page best; a section larger than a page goes
   where nothing fits. `I4004_PACK_INPUT_ORDER=1` keeps the input order (for
   comparison runs).
2. **Places and relaxes, to a fixed point.**
   - A `JCN`/`ISZ` lands on the page of the *next* instruction, so one at
     0xFE/0xFF reaches into the following page.
   - When the target is on another page, the branch becomes
     `JCN !cc, .+4; JUN target` (`ISZ` similarly), or is padded onto the next
     page when `.+4` itself would leave the page.
   - Relaxed skip operands are `PAGE8` relocations to local symbols
     (`_add_skip_reloc`), resolved after the last change, so sections can keep
     moving between passes.
   - Growing a section shifts its relocation sites, symbols and
     section-relative addends (`_shift_section`).
3. **Allocates RAM frames** (§3) and resolves all relocations.
4. **Checks the call depth** (§4).
5. **Writes the image and a map** (`firmware.map`: placement, ROM and RAM
   used).

The relaxation code has had five placement bugs, all found by running code at
arbitrary page offsets (`failure_notes/2026-10-22-llvm-i4004-packer-
relaxation.md`). A packer change that rewrites code needs an execution test
such as `system/busicom_141pf/firmware/scripts/exec_num.py`, not only lit/MC
tests.

**Banked images (frozen).** `--banks N` lays out a common region plus ROM
banks with far thunks and a bank latch. That was the 2026-10-21 path for the
~57 KB build. The flat image does not use it.

## 8. Runtime

`libi4004rt` (`rt/`, `abi.md` §6) supplies `crt0` and the operations that
don't expand inline: variable shifts, `i8` multiply/divide, memory copy and
fill over 4002 nibbles, and BCD instruction wrappers. The Busicom image calls
no bitwise runtime helpers; its arithmetic is in the title's own
`asm/num.s`.

## 9. Tests

`scripts/run_tests.sh` runs:
- **lit:** `tests/codegen` and `tests/mc`.
- **Execution checks on chips:** `exec_fn`, `exec_i16`, `exec_cmp`,
  `exec_nibble`.
- **The whole-firmware size gate:** `gen_worklist.py`; it fails if `.text` is
  over 4,096 B or any function over 1,024 B.

The title adds:
- `exec_num.py`: its assembly routines against their Rust twins, linked by
  the real packer.
- The 42-example image runs on chips and on the RTL.

## 10. Known weaknesses

- **AVR cost model:** a large share of the compiled Rust's size comes from
  inlining decisions tuned for AVR (§1).
- **Pseudo expansions are hand-written C++:** a gap that only shows across a
  real call boundary can pass every unit test. The `i1` return in the wrong
  pair is an example (`failure_notes/2026-10-22-llvm-i4004-bool-return-in-
  wrong-pair.md`).
- **No nibble-sized values:** `i4` values still occupy a full pair (issue #18
  task C2, not done).
