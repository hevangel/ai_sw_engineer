# llvm-i8008 code generation design

How Rust becomes an 8008 image. `abi.md` holds the contracts (data layout,
registers, calling convention, relocations); this file covers how the backend
meets them. It describes the backend as built. When the code changes, this
file changes with it (AGENTS.md).

## 1. Route

```
Rust crate ──rustc avr-none──▶ LLVM IR ──retarget_ir.py──▶ i8008 IR
   ──llc -mtriple=i8008──▶ ELF .o ─┐
   rt/*.s, platform asm ──llvm-mc──▶ .o ─┴─▶ i8008_link.py ──▶ image .bin + .map
```

- The pinned rustc has no 8008 target, so the crate is compiled for
  `avr-none` (8-bit, little-endian, 16-bit pointers) with `-Z build-std=core`,
  `opt-level=z`, `panic=abort` and fat LTO into one module.
  `rust/retarget_ir.py` rewrites the triple and data layout and strips AVR's
  code address space. It refuses floating point, `i128`, AVR intrinsics and
  data pointers outside address space 0. This is the same route as
  `llvm-i4004` (`llvm-i4004/rust/README.md`).
- `llc` and `llvm-mc` are built from the pinned `external/llvm-project`
  with the I8008 target overlaid (`scripts/apply_overlay.py`,
  `scripts/build_llvm.sh`).

## 2. Instruction selection

Legal types are i8 (`GR8`) and i16 (`GR16`). i16 is legal so that a pointer
stays one value; i16 arithmetic is still done one byte at a time, by the
expansions. i32 and i64 are split into i16 parts by the type legalizer.

Selection produces pseudo-instructions over `GR8`/`GR16` (`I8008InstrCodeGen.td`).
The register allocator only sees B, C, D, E, BC and DE. A, H and L are
reserved, and the expansions use them as scratch.

| IR | Pseudo | Expansion (typical) |
|---|---|---|
| `add/sub/and/or/xor i8` | `ALU8rr`, `ALU8ri` | `L A,x; op y; L d,A` |
| same, i16 | `ALU16rr`, `ALU16ri` | two bytes, carry through `AC`/`SB` |
| constant | `MOV8ri`, `MOV16ri` | `LdI` |
| `shl/lshr/ashr` by a constant | `SHIFT8`, `SHIFT16` | `ADA` / `ORA; RAR` chains |
| shift by a variable | libcall | `rt/shift.s` |
| `mul`, `div`, `rem` | libcall | `rt/arith.s` |
| `load`/`store` through a pointer | `LD8`, `ST8`, `LD16`, `ST16` | `L H,hi; L L,lo; L d,M` |
| frame slot | `LDF8`, `STF8`, spill/reload | `LHI %hi; LLI %lo; L d,M` |
| global address | `LEA_GA` | two `LdI` with `%lo`/`%hi` |
| frame address | `LEA_FI` | same, on `__frame_<fn>+off` |
| `br_cc` | `BRCC8`, `BRCC16`, `BRCC8ri` … | `CP`/`SU`/`SB` then `Jc` |
| `select_cc` | `SELECT_*` | branch diamond (custom inserter) |
| call / return | `CALL`, `CALLind`, `RETpseudo` | `CAL`, `RET` |

Compares use the borrow from `CP`/`SU`/`SB` for unsigned relations and `Z` for
equality. The 8008 has no overflow flag, so a signed compare first flips the
sign bit of both operands (`XRI 0x80`) and then compares unsigned.

## 3. Post-RA expansion

`I8008ExpandPseudos` turns each pseudo into real instructions. A is the
accumulator for every ALU step and H:L addresses every memory byte. Both are
dead between pseudos, so an expansion may clobber them freely. The flags are
dead between pseudos too: a compare and its branch are a single pseudo.

## 4. Frames, spills and arguments

- `FrameLowering` emits no prologue and no epilogue. Frame indices become
  offsets from `__frame_<fn>` (offsets are the real object layout, so
  multi-byte objects never alias).
- `AsmPrinter` publishes `__framesize_<fn>` and `__argsize_<fn>`.
- Memory arguments (`abi.md` §3.1) are stores to and loads from
  `__args_<callee> + off`.

## 5. H:L tracking (`I8008HLTrack`)

After expansion, a forward pass tracks what H and L hold: a symbol plus
offset, or a copy of a register pair. It deletes redundant `LHI`/`LLI`/`Lhr`
loads and turns a +1/−1 step within one object into `INL`/`DCL`. That is safe
because objects never cross a page (`abi.md` §4). The state is reset at block
entry and at every call.

## 6. The linker (`asm/i8008_link.py`)

- Reads the ELF objects and places sections in the regions named by a small
  linker script (one per image: boot ROM, monitor ROM, RAM programs). With
  `--gc-sections`, sections no root references are dropped.
- Resolves `R_I8008_*` relocations (`abi.md` §5).
- Builds the call graph from `R_I8008_16` relocations on `CAL`, then:
  rejects recursion, checks the call depth against `--max-depth`, and
  allocates `__frame_*` / `__args_*` by overlay (a function's area is placed
  above the areas of every function that can be live under it). No area
  crosses a 256-byte page.
- Writes the raw image, a map (symbols, frames, depth) and a size line.
