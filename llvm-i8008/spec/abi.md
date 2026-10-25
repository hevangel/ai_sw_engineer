# i8008 ABI

Written before the backend code (AGENTS.md). `rust/i8008-none.json` and
`I8008TargetMachine.cpp` must carry the data layout below character for
character. Everything here describes the backend as built; a change to the
calling convention, frames or relocations updates this file in the same change.

## 1. Data layout

```
e-m:e-p:16:8-i8:8-i16:8-i32:8-i64:8-n8-a:8
```

- Little-endian, ELF mangling.
- Pointers are 16 bits, byte-aligned. Only the low 14 bits address memory (the
  8008's H register drives address bits 13–8; bits 7–6 of H are ignored). Code
  and data share address space 0.
- Every integer type is byte-aligned. The only native width is 8 bits (`n8`).
- The `i32:8-i64:8` entries match the `avr-none` layout the Rust front end is
  run with (`rust/README.md`), so struct layouts and GEP offsets in the IR mean
  the same thing on both.

## 2. Registers

| Class | Registers | Allocatable | Use |
|---|---|---|---|
| `GR8` | B, C, D, E | yes | i8 values |
| `GR16` | BC (B high, C low), DE (D high, E low) | yes | i16 values and pointers |
| `ACC` | A | no | scratch for every ALU expansion |
| `HL` | H, L | no | the memory pointer: every `M` access goes through it |
| flags | C, Z, S, P | no | live only inside one expanded pseudo |

A and H:L are reserved because the 8008 can only compute in A and only reach
memory through H:L. Instruction selection works on pseudo-instructions over
`GR8`/`GR16`, which a post-RA pass expands into real 8008 code using A and H:L
as scratch (`codegen.md` §3).

No imaginary (memory-backed) registers: a fixed RAM address costs 5 bytes per
access (`LHI`, `LLI`, op), so spills go to the function's static frame (§4).

## 3. Calling convention

### 3.1 Arguments

- In order, each argument takes the first free register that fits:
  i8 → B, C, D, E; i16 or pointer → BC, DE (a whole pair, both halves free).
  An i32 is two i16 parts, low part first.
- Arguments that do not fit go to memory at `__args_<callee> + offset`. The
  caller stores them there before the `CAL`; the callee reads them from there.
  The linker reserves `__argsize_<callee>` bytes for the area and overlays it
  like a frame (§4). A call through a pointer cannot pass memory arguments
  (the compiler rejects it).
- `zeroext`/`signext` arguments arrive extended to the register width.

### 3.2 Return values

- i8 in B; i16 or pointer in BC; a second i16 (an `{i16, i16}` pair or the high
  half of an i32) in DE.
- Anything larger is returned through a hidden pointer argument (`sret`),
  which takes BC.
- A `zeroext`/`signext` i1 return is widened to i8.

### 3.3 Saved registers

None. A, B, C, D, E, H, L and the flags are all clobbered by a call. A value
live across a call is kept in the caller's static frame, or in a register
the callee is known not to touch (IPRA, `codegen.md` §4).

One exception by name: a hand-written routine called `__leaf_<name>` clobbers
only A, the flags and its result register. It keeps C, D, E, H and L, and B
too when it returns nothing (`CSR_I8008_Leaf`, `CSR_I8008_LeafVoid`). Device
drivers use it (`system/micral_n/platform/runtime/tty.s`): the caller keeps
its registers and H:L across the call. A routine with this prefix must keep
the promise; one that needs more registers takes an ordinary name.

### 3.4 Calls and returns

- Direct calls are `CAL target` (3 bytes); returns are `RET`.
- The 8008's on-chip stack has 7 usable return levels. The linker computes the
  static call depth of the whole image, counting runtime helpers, and fails the
  link above the limit set for the image (7 by default; the monitor reserves
  levels for the user program it runs). Recursion fails the link.
- A restart point (linker `RESET`, `codegen.md` §6) may be called or jumped
  to from any depth and never returns: the stack is circular, so the levels
  it abandons are overwritten harmlessly. Code after the restart starts a
  new chain of 7 levels.
- Indirect calls go through the RAM trampoline `__icall` (a 3-byte `JMP`
  whose address bytes the caller patches). The linker treats an indirect call
  as a call to every address-taken function.

## 4. Frames

There is no data stack. Every function that needs memory for locals, spills or
`alloca` gets a **static frame** in RAM:

- The code addresses it as `__frame_<fn> + offset`. The size is published as
  the absolute symbol `__framesize_<fn>` (bytes).
- The linker places frames by walking the call graph. Two functions whose
  activations can be live at the same time never share bytes; the others
  overlay. This is safe only because nothing is reentrant: no recursion, no
  interrupts.
- No frame, argument area or other object of up to 256 bytes crosses a
  256-byte page boundary; every data object is a section of its own
  (`codegen.md` §1), so the rule holds per object. Code may then step H:L through an object with
  `INL` / `DCL` alone (`codegen.md` §5).

## 5. Object format and relocations

ELF32, little-endian, machine `EM_I8008 = 0x8008` (private, unregistered),
`ELFOSABI_STANDALONE`, `RELA` relocations.

| Relocation | Value | Field | Use |
|---|---|---|---|
| `R_I8008_NONE` | 0 | — | — |
| `R_I8008_16` | 1 | 2 bytes, little-endian | `JMP`/`CAL`/`Jc`/`Cc` address; `.word` data |
| `R_I8008_LO8` | 2 | 1 byte | `LLI %lo(sym)` |
| `R_I8008_HI8` | 3 | 1 byte | `LHI %hi(sym)`: bits 15–8 of the address |
| `R_I8008_8` | 4 | 1 byte | `.byte sym` (low 8 bits, range-checked) |

The linker is `asm/i8008_link.py`, a Python script like the i4004 packer. It
places sections at the addresses in the linker script, allocates frames (§4),
checks call depth, and writes a raw binary image plus a map.

## 6. Runtime library (`rt/`)

LLVM calls these hand-written assembly helpers. All of them use the convention
in §3, clobber what §3.3 says, and use no frame of their own unless noted.

| Helper | Operation |
|---|---|
| `__mulqi3`, `__mulhi3`, `__mulsi3` | multiply i8 / i16 / i32 |
| `__udivqi3` `__umodqi3` `__divqi3` `__modqi3` | i8 divide / remainder |
| `__udivhi3` `__umodhi3` `__divhi3` `__modhi3` | i16 divide / remainder |
| `__udivsi3` `__umodsi3` `__divsi3` `__modsi3` | i32 divide / remainder |
| `__ashlqi3` `__lshrqi3` `__ashrqi3` (and `hi3`) | shift by a variable amount |
| `memcpy(dst, src, n)`, `memmove(dst, src, n)` | C semantics: dst in BC, src in DE, n a memory argument |
| `memset(dst, c, n)` | dst in BC, c in D, n a memory argument |

The registers follow §3.1 from the types SelectionDAG gives the libcall:
`memset`'s value is an i8 (the width of the IR value), so after the pointer
in BC it takes D, not DE. The i32 helpers take two i32s, so the second
operand is a memory argument in `__args_<helper>`.
