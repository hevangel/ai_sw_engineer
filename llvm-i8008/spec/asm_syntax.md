# i8008 assembler syntax

`llvm-mc -triple i8008` and the code the compiler prints use this syntax: the
original 1972 Intel mnemonics (`isa.md`), with GNU `as` directives so the LLVM
MC layer and lit/FileCheck work unchanged.

## Lines

`[label:] mnemonic [operand] [; comment]`. Mnemonics are case-insensitive;
the compiler prints them in lower case. `#` also starts a comment.

## Mnemonics

The register names are part of the mnemonic, as in the MCS-8 manual:

| Form | Examples |
|---|---|
| `l<d><s>`, `l<d>m`, `lm<s>` | `lab`, `lam`, `lma`, `lhd` |
| `l<d>i imm`, `lmi imm` | `lai 0x0d`, `lmi 0` |
| `in<d>`, `dc<d>` (d = b c d e h l) | `inl`, `dcb` |
| `<op><s>`, `<op>m`, `<op>i imm` with op in `ad ac su sb nd xr or cp` | `adb`, `cpm`, `ndi 0x7f` |
| `rlc rrc ral rar` | |
| `jmp`/`cal addr`; `jfc jfz jfs jfp jtc jtz jts jtp addr`; `cfc` … `ctp addr` | `jtz loop` |
| `ret`; `rfc` … `rtp` | `rtz` |
| `rst n` (0–7), `inp p` (0–7), `out p` (8–31), `hlt` | `out 8` |

## Operands

- Immediates are 8-bit: −128 … 255 (a negative value is stored modulo 256).
- Integers use the LLVM lexer: `0x1f`, `0b101`, `017` (a leading 0 means
  octal, which suits listings written in the manual's octal), or decimal.
- Addresses are labels, integers or expressions, 0 … 0x3fff.
- `%lo(expr)` and `%hi(expr)` give the low and high byte of an address, for
  `lli`/`lhi` and the other immediate forms:
  `lhi %hi(buf)` / `lli %lo(buf)`.

## Directives

The usual ELF directives: `.text`, `.data`, `.bss`, `.section`, `.globl`,
`.byte`, `.word` (2 bytes, little-endian, may name a symbol), `.ascii`,
`.asciz`, `.zero`, `.set`, `.p2align`.
