# i4004 ABI

Written before any backend code. `rust/i4004-none.json` must match it exactly.

## Data layout

- LLVM data layout string: `TODO`
- Pointer width / address spaces (code vs data, banked ROM):
- Native integer widths (`n...`), alignment of each type:
- Endianness:

## Registers

| Class | Registers | Allocatable | Notes |
|---|---|---|---|

Imaginary registers (memory-backed pseudo-registers, as llvm-mos does with zero page): TODO.

## Calling convention

- Arguments:
- Return values:
- Callee-saved:
- Stack: direction, pointer, frame layout, hardware stack depth limits:

## Relocations and object format

TODO (ELF machine number / custom, relocation kinds).

## Runtime library

Helpers LLVM will call (multiply, divide, shifts by variable, memcpy) and where they live.
