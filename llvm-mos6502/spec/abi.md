# MOS 6502 ABI Specification

Documented in accordance with LLVM-MOS architecture conventions and `AGENTS.md`. `rust/mos6502-none.json` matches this specification exactly.

## 1. Data Layout

LLVM Data Layout String:
```
e-m:e-p:16:8-p1:8:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-Fi8-n8
```

- **Endianness (`e`)**: Little-endian.
- **Mangling (`m:e`)**: ELF-style mangling.
- **Pointers (`p:16:8`)**: Standard data and function pointers are 16 bits (addressing 64 KiB), aligned to 8 bits.
- **Zero-Page Pointers (`p1:8:8`)**: Address space 1 (`AS 1`) pointers represent Zero Page ($0000–$00FF), 8 bits wide, aligned to 8 bits.
- **Integers (`i16:8-i32:8-i64:8`)**: All multi-byte integers are aligned to 8-bit boundaries.
- **Aggregates (`a:8`)**: Structs and arrays have 8-bit alignment.
- **Native Integer Width (`n8`)**: 8 bits. The ALU operates on 8-bit quantities (`A` register).

---

## 2. Hardware and Imaginary Registers

### 2.1 Hardware Registers
The MOS 6502 has very few hardware registers:
- `A` (8-bit): Accumulator. Primary ALU operand and result register.
- `X` (8-bit): Index register. Supports zero-page, absolute, and stack pointer indexing.
- `Y` (8-bit): Index register. Supports zero-page and indirect indexed (`(zp),Y`) addressing.
- `SP` (8-bit): Hardware stack pointer. Hardwired to page 1 ($0100–$01FF), grows downward from $01FF.
- `P` (8-bit): Processor status flags (`N`, `V`, `-`, `B`, `D`, `I`, `Z`, `C`).
- `PC` (16-bit): Program counter.

### 2.2 Imaginary Registers (Zero-Page Virtual Registers)
Following the LLVM-MOS model, LLVM allocates a contiguous window of Zero-Page bytes as "imaginary registers" (virtual registers `__rc0` through `__rc31`):
- Pointers, 16-bit, 32-bit arithmetic, and register allocator spills reside in these fast Zero-Page memory locations.
- Zero-page imaginary registers allow standard SSA register allocation and instruction selection for multi-byte expressions.
- The platform linker script (`link.ld`) reserves these Zero-Page locations to avoid collisions with the target OS/Monitor.

---

## 3. Calling Convention

### 3.1 Return Values
- **8-bit (`u8`, `i8`, `bool`)**: Returned in `A`.
- **16-bit (`u16`, `i16`, pointers)**: Returned in `A` (low byte) and `X` (high byte).
- **32-bit / larger aggregates**: Returned via zero-page imaginary registers or caller-allocated `sret` buffer pointer.

### 3.2 Argument Passing
- First 8-bit argument: passed in `A`.
- Second 8-bit argument: passed in `X`.
- Third 8-bit argument: passed in `Y`.
- 16-bit and multi-byte arguments: passed in zero-page imaginary registers (`__rc2`, `__rc3`, etc.) or pushed onto the soft stack.

### 3.3 Callee-Saved vs. Caller-Saved Registers
- Hardware registers `A`, `X`, `Y`, and status flags `P` are **caller-saved** (clobbered by calls).
- Zero-page imaginary registers designated as callee-saved must be preserved by function prologues/epilogues when used across calls.
- Hardware stack pointer `SP` is preserved.

### 3.4 Stack Organization
- **Hardware Stack**: Page 1 ($0100–$01FF). Used by `JSR` and `RTS` for 16-bit return addresses, and `PHA`/`PLA`. Maximum call depth is ~128 levels (substantially larger than the 4004's 3 levels).
- **Software Stack (Data Stack)**: When local variables and frame spills exceed available Zero-Page imaginary registers, a downward-growing software stack is maintained via a zero-page pointer.

---

## 4. Runtime Library (`libcompiler_builtins`)

The runtime provides essential multi-byte arithmetic and memory intrinsics:
- 16-bit & 32-bit multiplication: `__mulhi3`, `__mulsi3`
- 16-bit & 32-bit division & modulo: `__udivhi3`, `__umodhi3`, `__divhi3`, `__modhi3`
- Shifts: `__ashlhi3`, `__lshrhi3`, `__ashrhi3`
- Memory helpers: `memcpy`, `memset`, `memmove`
Provided directly by `mrk-its/compiler-builtins` (branch `mos-0.1.148`) in the toolchain.
