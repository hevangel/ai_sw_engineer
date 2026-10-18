# i4004 assembler syntax

The integrated LLVM MC assembler (and `llvm-mc`) accepts this syntax; it
follows the Intel MCS-4 manual's mnemonics and the register/operand
conventions used by the ai_hw_engineer tooling, with GNU-`as`-style directives
so `ld.lld` and the lit/FileCheck harness work unchanged.

## Mnemonics and operands

- One instruction per line: `[label:] MNEMONIC [operands] [; comment]`.
- Mnemonics are the uppercase MCS-4 names from `isa.md` (`NOP`, `JCN`, `FIM`,
  `SRC`, `FIN`, `JIN`, `JUN`, `JMS`, `INC`, `ISZ`, `ADD`, `SUB`, `LD`, `XCH`,
  `BBL`, `LDM`, the `OPR=0xE` RAM/ROM group, the `OPR=0xF` accumulator group).
  The assembler is case-insensitive on mnemonics and register names.
- **Index registers:** `R0`–`R15`. **Register pairs:** `P0`–`P7` (alias
  `R0R1`…`R14R15` accepted). `FIM`/`SRC`/`FIN`/`JIN` take a **pair**; `INC`,
  `ISZ`, `ADD`, `SUB`, `LD`, `XCH` take a single **register**.

Operand forms, matching the manual's assembler examples:

| Instruction | Syntax | Example |
|---|---|---|
| single-register | `MNEM Rn` | `ADD R5` |
| pair | `MNEM Pn` | `SRC P2` |
| `FIM` pair, imm8 | `FIM Pn, imm8` | `FIM P1, 0x3C` |
| `LDM`/`BBL` imm4 | `MNEM imm4` | `LDM 9` |
| `JUN`/`JMS` addr12 | `MNEM addr` | `JUN start` |
| `JCN` cond, addr8 | `JCN cond, addr` | `JCN Z, loop` or `JCN 0x4, loop` |
| `ISZ` reg, addr8 | `ISZ Rn, addr` | `ISZ R3, loop` |

- **`JCN` condition** may be a hex nibble (`0x0`–`0xF`) or a mnemonic combo
  built from the §4 bits of `isa.md`: `Z` (ACC=0, C2), `C` (CY=1, C3), `T`
  (TEST=0, C4), and a leading `N` to set the invert bit C1 — e.g. `NZ`, `NC`,
  `TZ`. `JCN 0x8, addr` (invert, no condition) is the canonical unconditional
  in-page jump; the assembler also accepts the pseudo `JMP addr` for it.
- **Immediates:** `imm4` is 0–15, `imm8` is 0–255; out-of-range is an error.
- **Addresses** are labels or absolute integers. For `JCN`/`ISZ`/`FIN` (page-
  local, `isa.md` §9) the assembler/linker verify the target is on the current
  256-byte page and error with the end-of-page diagnostic otherwise.

## Numeric literals

- Decimal: `255`. Hex: `0x1F` or `1Fh` (both accepted; `0x` canonical). Binary:
  `0b1010`. Octal: `0o17`. Character: `'A'` (8-bit value).
- Current location counter: `$` (byte offset within the current section).

## Directives (GNU-as compatible subset)

| Directive | Meaning |
|---|---|
| `.text` / `.rodata` / `.data` / `.bss` | select output section (ROM: `.text`,`.rodata`; RAM: `.data`,`.bss`) |
| `.section NAME` | select/declare a named section |
| `.reset` | the reset vector section placed at ROM 0x000 (`KEEP`ed by the linker script) |
| `.byte v, …` | emit 8-bit bytes |
| `.nibble v, …` | emit a single 4-bit value (packs two per byte, high nibble first) |
| `.word v, …` | emit a 12-bit ROM address (for `FIN` tables / jump tables), stored per `R_I4004_ROMADDR12` |
| `.ascii` / `.asciz STR` | emit character bytes (optionally NUL-terminated) |
| `.align N` | align the location counter to an N-byte boundary (page alignment uses `.align 256`) |
| `.page` | shorthand for `.align 256` — start a new 4001 page (used around page-local branch regions) |
| `.globl SYM` / `.local SYM` | symbol visibility |
| `.equ NAME, expr` / `NAME = expr` | symbol constant |
| `.org EXPR` | set the location counter (within a section) |
| `.space N [, fill]` | reserve N bytes |

## Comments

- `;` to end of line (MCS-4 manual style). `#` and `//` are also accepted; `/* … */` block comments are accepted for generated code.

## Labels

- `name:` at line start defines a label at the current location. Local labels
  `1:` … referenced as `1f`/`1b` (forward/back) are accepted for compiler
  output.

## Example

```asm
        .reset
start:  FIM  P0, 0x00        ; pair 0 = pointer base
        LDM  0               ; ACC = 0
        XCH  R4              ; R4 = 0 (loop counter)
loop:   SRC  P0              ; set RAM address from pair 0
        RDM                  ; ACC = RAM[P0]
        DAA                  ; decimal adjust
        WRM                  ; store back
        ISZ  R4, loop        ; R4++ ; branch in-page until R4 wraps to 0
        BBL  0               ; return, ACC = 0
```

Source: Intel MCS-4 Users Manual assembler conventions (mnemonics, `;`
comments, register/pair operand forms — see `isa.md` §10); GNU-as directive
names for `ld.lld`/lit compatibility; page-local operand checking per `isa.md`
§9 and the relocations in `abi.md` §5.
