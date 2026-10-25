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
  `opt-level=z`, `panic=abort` and fat LTO into one module
  (`scripts/build_rust.sh`). `-C llvm-args=-indvars-widen-indvars=false`
  keeps loop counters at the width the source gives them: a `u8` counter
  widened to i16 costs a byte pair on every step.
- `rust/retarget_ir.py` rewrites the triple and data layout and strips AVR's
  code address space. It refuses floating point and `i128` types (a type
  keyword, not part of a value name such as `%x.i128`), AVR intrinsics and
  data pointers outside address space 0 (a code address taken as an
  integer, such as the bytes of a patched `CAL`, is not one). This is the
  same route as
  `llvm-i4004` (`llvm-i4004/rust/README.md`).
- Every function and every data object gets its own section: the
  `I8008TargetMachine` turns on function and data sections whatever the
  command line says. The linker drops unused sections, sees one function
  per section in the call graph, and keeps each object of up to 256 bytes
  inside one page, which selection relies on (§2, "Page-bounded objects").
  `LLC_FLAGS` passes per-image options (`-i8008-data-page`, §5).
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
| `shl/lshr/ashr` by a constant | `SHL/SRL/SRA 8ri/16ri` | `ADA` / `ORA; RAR` chains; a long shift is a short rotate the other way plus a mask (`x >> 7` = `RLC; NDI 1`) |
| shift by a variable | libcall | `rt/shift.s` |
| `mul`, `div`, `rem` | libcall | `rt/arith.s`; `x * c` for c = 2^k, 2^k ± 1 (k ≤ 4) becomes shifts and adds (`decomposeMulByConstant`) |
| `load`/`store` through a pointer | `LD8`, `ST8`, `LD16`, `ST16` | `L H,hi; L L,lo; L d,M` |
| `load`/`store` at a link-time address | `LDA8`, `STA8`, `LDA16`, `STA16` (`AbsAddr`) | `LHI %hi; LLI %lo; L d,M` |
| frame slot | spill/reload, frame index → `LEA16` | `LHI %hi; LLI %lo; L d,M` on `__frame_<fn>+off` |
| global address | `LEA16` (`I8008ISD::WRAPPER`) | two `LdI` with `%lo`/`%hi` |
| `load`/`store` at small-global + index | `LDPG8`, `STPG8`, `STPG8i`; `LEAPG` as a value | `L A,i; ADI %lo(g); L L,A; LHI %hi(g); L d,M` |
| `%lo` / `%hi` of a global as a byte | `MOV8sym` (`I8008ISD::WRAPPER8`) | `LdI %hi(g)` |
| `br_cc` | `BRCC8rr/ri`, `BRCC16rr/ri` | `CP`/`SU`/`SB` then `Jc` |
| `setcc` (a compare as a value) | `SETCC8rr/ri`, `SETCC16rr/ri` | the borrow or a zero test, then `LAI 0; ACI 0` (no branch) |
| `select_cc` | `SELECT_*` | branch diamond (custom inserter) |
| call / return | `CALL`, `CALLind`, `RETpseudo` | `CAL`, `RET` |

Compares use the borrow from `CP`/`SU`/`SB` for unsigned relations and `Z` for
equality. The 8008 has no overflow flag, so a signed compare first flips the
sign bit of both operands (`XRI 0x80`) and then compares unsigned. Constant
operands of a compare are moved to the right, where the `ri` forms take them.
No jump tables: the 8008 has no indirect jump.

Compares as values. The 8008 has no set-on-condition, so a compare whose
result is a value (a `bool` returned or stored) would be a branch diamond
and a frame slot. `LowerSETCC` reduces every relation to EQ, NE, ULT or UGE
(signed ones flip the sign bits, `>` and `<=` swap the operands or add 1 to
a constant) and `SETCC*` expands without branches: `L A,a; SU b` leaves the
borrow of a - b in the carry (ULT; UGE adds `XRI 1`), or a zero test of the
difference (16 bits: the XOR of the halves ORed) is turned into the carry by
`SUI 1` (EQ: carry iff zero) or `ADI 0xFF` (NE: carry iff nonzero). Then
`LAI 0; ACI 0` makes A = carry; loads keep the flags. A compare against 0 is
just `L A,lo; OR hi`. A compare that only feeds a branch stays a `BRCC`.

Page-bounded objects. A load or store at `g + y` (y not a constant), where
g is a global of at most 256 bytes, is inside g: LLVM's memory model lets a
pointer based on g reach only g. The linker never lets such an object cross
a page (one section per object, §1, §6), so the high byte is g's and only
the low byte needs the add (`pageIndexedAddress`, `I8008ISD::PGADDR`):
`L A,y; ADI %lo(g); L L,A; LHI %hi(g)` instead of a 16-bit add into a pair.
Larger globals (SCELBAL's RAM image) use the alignment rule below.

Byte pairs. An i16 `or`/`add` whose operands have disjoint bytes (one with a
known-zero low byte, the other with a known-zero high byte) becomes
`BUILD_PAIR(low of one, high of the other)`: two register moves instead of a
16-bit add (`PerformDAGCombine`). Two things feed it:

- The TTI hook `isProfitableToSinkOperands` sinks `shl (zext x), 8` into the
  block of the `or`/`add` that uses it, so the combine sees both operands.
- `computeKnownBitsForTargetNode` gives the address of a global its known low
  bits from the global's alignment. For a 256-aligned table, `&t[i]` with an
  i8 index is `H = page of t, L = i`. Titles align their page-structured
  data for this (`system/micral_n/basic/src/mem.rs`).

The register coalescer never merges a GR8 copy with a subregister of a GR16
(`shouldCoalesce`): with four allocatable bytes, such merges ran the
allocator out of registers.

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
  `__args_<callee> + off`. `LowerCall` creates the `__args_<callee>` symbol.
- Interprocedural register allocation (IPRA) is on: a call's register mask
  is what the callee really clobbers, so a caller keeps values in registers
  across calls to small functions instead of spilling them to its frame.
  Calls to `__leaf_*` routines use the fixed masks of `abi.md` §3.3.

## 5. H:L and A tracking (`I8008HLTrack`)

After expansion, a forward pass over each block tracks what H and L hold: a
symbol plus offset, or a copy of a register pair. It deletes redundant
`LHI`/`LLI`/`Lhr` loads and turns a +1/−1 step within one object into
`INL`/`DCL`. That is safe because objects never cross a page (`abi.md` §4).
A block with one predecessor starts from that predecessor's exit state; any
other block starts empty. A call forgets everything, except a call to a
`__leaf_*` routine, which keeps H:L.

A second walk tracks which register A is a copy of, and deletes `L A,r` when
A already holds r and `L r,A` when r already equals A. In data-page mode a
lone `LHI %hi(g)` (the page of a page-indexed access) is dropped when H
already holds the data page and g is in it. A third peephole
drops `L A,r; ORA` before a zero/sign/parity branch when the instruction just
before was `INr`/`DCr`, which set those flags from r already.

With `llc -i8008-data-page`, the image keeps every writable object (frames,
argument areas, `.data`, `.bss`) in one 256-byte page, so an `LHI` for any of
them is redundant whenever H already holds that page. The linker's
`DATAPAGE` directive checks the promise (§6). The monitor ROM uses it.

## 6. The linker (`asm/i8008_link.py`)

- Reads the ELF objects and places sections in the regions a small layout
  file names (one per image). Directives:

  | Directive | Meaning |
  |---|---|
  | `CODE start size` | `.text*`, `.rodata*` |
  | `DATA start size` / `DATA follow size` | `.data*`, `.bss*`, frames, argument areas; `follow` puts them right after the code, in the image |
  | `ENTRY sym` | placed first; the root of the call graph and of section GC |
  | `KEEP sym` | another GC root |
  | `PLACE sym addr` | a fixed entry point; the rest of CODE flows around it |
  | `DEPTH n` | maximum call depth (default 7) |
  | `DATAPAGE` | all writable objects in one page (`-i8008-data-page`, §5) |
  | `RESET sym` | a restart point (below) |

  Sections nothing reachable references are dropped. Sections keep their
  ELF alignment (a 256-aligned table starts a page). Objects of up to 256
  bytes never cross a page. Code goes into the free intervals that the
  `PLACE`d sections leave: the `ENTRY` section first at the start of CODE
  (unless placed), then best fit, largest first, so the holes in front of
  fixed entry points fill up (the monitor's ROM has two).
- Resolves `R_I8008_*` relocations (`abi.md` §5).
- Builds the call graph from `R_I8008_16` relocations on `CAL`/`Cc` (a call)
  and on `JMP`/`Jc` into another function (a tail jump: an edge with no stack
  level of its own). A section's function is its `STT_FUNC` symbol, or for
  hand-written assembly the label at offset 0. Then it rejects recursion,
  checks the call depth against `DEPTH`, and allocates `__frame_*` /
  `__args_*` by overlay (a function's area is placed above the areas of
  every function that can be live under it).
- `RESET sym`: code jumps or calls to `sym` from any depth and never returns
  (a title's error restart, as SCELBAL's `JMP EXEC`). The 8008's return
  address stack is a circular register file, so the levels left behind are
  simply overwritten. Calls to `sym` are not graph edges; `sym` is a root of
  its own, with a fresh 7-level budget.
- Writes the raw image, a map (sections, symbols, frames, depth) and a size
  line.
