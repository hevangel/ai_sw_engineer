# llvm-i8086 runtime library

The routines that `llc` calls for what the 8086 has no instruction for. They use the calling convention of [../spec/abi.md](../spec/abi.md) D3 (arguments in AX, DX, CX then the stack, results in AX or DX:AX, BX SI DI BP preserved), and the libgcc names the backend registers (`I8086Subtarget::initLibcallLoweringInfo`).

| Source | Routines | Written in |
|---|---|---|
| `asm/divsi.s` | `__udivsi3`, `__umodsi3`, `__divsi3`, `__modsi3` (32-bit divide and remainder); `__ashlsi3`, `__lshrsi3`, `__ashrsi3` (32-bit shifts by a count in CL); `__mulsi3`; `__i8086_udivmodsi` (internal) | assembly |
| `asm/mem.s` | `memcpy`, `memmove`, `memset`, `memcmp`, `bcmp` | assembly |
| `ir/di.ll` | `__muldi3`, `__udivdi3`, `__umoddi3`, `__divdi3`, `__moddi3`, `__ashldi3`, `__lshrdi3`, `__ashrdi3` (64-bit) | LLVM IR, compiled by `llc` |

| `obj/compiler_builtins.o` | IEEE 754 single and double arithmetic, comparisons and conversions (`__addsf3`... `__floatundidf`), 128-bit multiply, divide, remainder and shifts (`__multi3`, `__udivti3`...), and weak `mem*` | Rust's `compiler_builtins`, built by `scripts/build_compiler_builtins.sh` (rustc as msp430, the IR filtered of libm and half/fp128 functions by `scripts/ir_drop_functions.py`, our `llc`); weak definitions, so the routines above win |

Each routine is in a section of its own (`.text.<name>`), so a linker that drops unreferenced sections keeps only what a program uses. `scripts/build_rt.sh` builds `obj/*.o` (committed; `run_tests.sh` checks they are fresh).

## Contracts

- **32-bit values** are a low word then a high word, so for `(a, b)` the dividend `a` is DX:AX, the divisor's low word is CX and its high word is the first stack word, `[sp+2]` at entry. The result is DX:AX. Division by zero executes the chip's divide error (interrupt 0); `INT_MIN / -1` is undefined, as in LLVM IR.
- **`__i8086_udivmodsi`** is internal: dividend DX:AX, divisor BX:CX; quotient DX:AX, remainder BX:CX; SI, DI and BP preserved. A divisor under 65536 is two 32/16 `div`s. Otherwise the quotient is under 65536 and is found with one `div` of the normalised operands plus a correction of at most one (Warren, *Hacker's Delight*, "divide unsigned, long by long", with 16-bit words); no loop runs per quotient bit.
- **Shifts** take the count in CL (a byte; the upper half of CX is undefined) and use it modulo 32.
- **64-bit values** are four words. A 64-bit result is returned through a hidden pointer passed first in AX (the compiler's rule for results that do not fit DX:AX), so `__udivdi3(n, d)` takes the pointer in AX, `n` in DX, CX and two stack words, and `d` in four stack words; the IR definitions get this from the compiler. Shift counts are a byte, used modulo 64. Unsigned division by zero does not trap: the quotient is all ones and the remainder is the dividend (the signed forms divide the magnitudes and then fix the signs, so their result for a zero divisor is not specified).
- **`memcpy`/`memset`** return the destination. `memset` fills with the low byte of its second argument. **`memmove`** copies forward unless the destination starts inside the source, then backward from the end with the direction flag set (and cleared again before returning). **`memcmp`/`bcmp`** return the difference of the first unequal bytes taken as unsigned, or 0; `bcmp` is the same code. The direction flag is clear on entry and exit of every routine.
- No routine uses a segment override (the 8086/8088 erratum that loses all but one prefix of an interrupted `rep` is not triggered).

## Verified

`system/ibm_pc/emulator/tests/i8086_codegen.rs` links `tests/run/rt.o` (compiled calls into every routine) with `obj/*.o` and runs them on the 8088 core against host arithmetic: 3,721 operand pairs for each 32-bit divide and remainder (edge values and spreads of magnitude, covering the 16-bit-divisor path, the normalised path and the correction), every shift count 0..31 for the 32-bit shifts, 256 pairs for each 64-bit operation, 14 shift counts for each 64-bit shift, `memmove` over every overlap of a 20-byte move, and `memcmp` at every byte position. Not verified: division by zero, `INT_MIN / -1`, and any timing.
