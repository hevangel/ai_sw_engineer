# i8086 assembler syntax

- Mnemonics and operand syntax: Intel mnemonics and operand order (destination first), as in the 8086 manual (`mov ax, [bx+si+2]`, `add word [di], 5`). Operand size is given by a register operand or an explicit `byte`/`word` keyword; a memory operand without either is an error. Segment overrides are written in the operand (`es:[di]`).
- Directives: `.text`, `.data`, `.bss`, `.section name`, `.globl sym`, `.byte`, `.word`, `.ascii`, `.asciz`, `.zero n`, `.align n`, `.org addr` (boot sector layout). 
- Comment character: `;`
- Numeric literals: decimal, `0x` hex, `0b` binary; a trailing `h` is not accepted, to avoid ambiguity with labels.
- Branch relaxation: `jcc target` that is out of short range is assembled as an inverted `jcc` over a `jmp`; `jmp` chooses the 2-byte or 3-byte form. Both are normal, documented assembler behavior and are covered by MC tests.
