# i8086 assembler syntax

Intel syntax (destination first), as implemented by `llvm-mc -triple=i8086`
(`target/lib/Target/I8086/AsmParser/`). The table of what each instruction
assembles to is `tests/mc/encodings.s`; the instruction forms are in `spec/isa.md`.

## Lexical

- Comment character `;`. A line may also begin with `#` (a comment). `#` after an instruction is *not* a comment.
- Mnemonics, register names and the keywords `byte`, `word`, `dword`, `ptr`, `short`, `near` are case-insensitive.
- Numbers: decimal, `0x` hexadecimal, `0b` binary. There is no `h` suffix. Expressions with `+ - * /`, parentheses and symbols follow the generic LLVM assembler.
- A label may not be called `short`, `near` or `far`, or a register or mnemonic name.

## Operands

| Operand | Syntax | Notes |
|---|---|---|
| register | `ax cx dx bx sp bp si di`, `al cl dl bl ah ch dh bh`, `es cs ss ds` | `cs` cannot be loaded (`mov cs, ax`, `pop cs` are errors) |
| immediate | `5`, `-1`, `0x1234`, `label`, `label+4` | a bare name is the address of the symbol, as in NASM |
| memory | `[bx+si+4]`, `[bp-2]`, `[di]`, `[0x1234]`, `[label]`, `[bx+label]` | always bracketed; base `bx`/`bp`, index `si`/`di`, any displacement; the only forms the hardware has |
| size | `byte ptr [bx]`, `word ptr [bx]`, `dword ptr [bx]` | needed when no register operand fixes the size (`mov word ptr [bx], 5`, `inc byte ptr [di]`); `ptr` is optional |
| segment override | `es:[bx]`, `byte ptr cs:[bx+4]` | emitted as a prefix byte before the instruction |
| far pointer | `0x1000:0x0100` | segment:offset, for `jmp` and `call` |
| branch target | `label`, a constant | a constant is an absolute address in the section |

`jmp short L` and `jmp near L` fix the form; a bare `jmp L` and `jcc L` start short and are relaxed (below).

## Instruction choices (the assembler's encoding rules)

- Register-to-register ALU, `mov` and `xchg` use the `op r/m, reg` opcode (`add bx, cx` is `01 CB`, not `03 D9`). The decoder accepts both and prints both as `add`.
- `op al, imm8` and `op ax, imm16` use the short accumulator opcodes when the immediate does not fit a signed byte (or is a symbol); a 16-bit immediate that does fit uses the sign-extended `83` form (`add ax, 5` is `83 C0 05`).
- `mov reg, imm` uses `B0+r`/`B8+r`; `mov mem, imm` uses `C6`/`C7`. `mov al/ax, [direct]` and `mov [direct], al/ax` use `A0-A3`.
- `xchg ax, reg` uses the one-byte `90+r`.
- `int 3` is `CD 03`; `int3` is the one-byte `CC`.
- `sal` is `shl`; `jz jnz jc jnc jnae jnb jna jnbe jpe jpo jnge jnl jng jnle` and `loopz loopnz retn repe repz repnz xlatb` are aliases.

## Prefixes

`rep`, `repe`/`repz` (F3), `repne`/`repnz` (F2), `lock` (F0) and the segment prefixes `es cs ss ds` are instructions of their own and may share a line with the next: `rep movsb` is two instructions. The disassembler folds a segment prefix into the memory operand that follows when there is one (`es:[bx]`) and prints it alone otherwise.

## Branch relaxation

The conditional jumps reach only -128..127 bytes. `jcc L` (no `short`) is assembled as the two-byte form and, when the target is out of range or an external symbol, as the five-byte `j!cc +3 ; jmp near L`. A bare `jmp L` grows from two bytes (`EB`) to three (`E9`). `loop`, `loope`, `loopne`, `jcxz` and `jcc short`/`jmp short` have only the short form and report "branch target out of range for a short jump". `call L` is always the three-byte near call.

## Relocations

`R_I8086_16` (16-bit absolute), `R_I8086_8` (8-bit absolute), `R_I8086_PC16` (near `jmp`/`call`, addend -2), `R_I8086_PC8` (short jumps, addend -1); see `spec/abi.md`.

## Directives

The generic ELF directives (`.text .data .bss .section .globl .byte .short .ascii .asciz .zero .space .align .org .set`), plus `.word expr[, expr...]` for 16-bit data.

## Disassembler output

`llvm-mc --disassemble` and `llvm-objdump -d` print the same syntax: sizes on every memory operand except `lea`, numbers below ten in decimal and otherwise hexadecimal, branch targets as absolute addresses (`jmp short 0x7`), far pointers as `segment:offset`.
