; Start of a medium-model PC DOS program (.EXE, llvm-i8086/spec/abi.md D10).
;
; i8086-ld --exe makes CS = SS = the PSP's segment (the first segment's offsets run from 100h)
; and SP = __stack_top; DOS enters with DS = ES = the PSP. This code
;   1. keeps the PSP's segment in __psp and AX (the drive-validity flags) in __entry_ax;
;   2. clears .bss (in the first segment);
;   3. calls main far (main is `cc 1000` like every Rust function of the medium model);
;   4. ends the program with INT 21h function 4Ch, code 0, if main returns.
; The stack is __MEDIUM_STACK bytes (4 KiB) of .bss; __prog_end, in the section .endmark that
; the generated linker script puts last, marks the end of the program's memory (far_shrink).

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	cld
	mov	bx, ax
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	mov	word ptr [__psp], ds
	mov	word ptr [__entry_ax], bx
	call	main@seg:main
	mov	ax, 0x4c00
	int	0x21

	.section	.bss.__psp,"aw",@nobits
	.globl	__psp
__psp:
	.zero	2
	.globl	__entry_ax
__entry_ax:
	.zero	2

	.section	.bss.__medium_stack,"aw",@nobits
	.zero	4096
	.globl	__stack_top
__stack_top:

	.section	.endmark,"aw",@progbits
	.globl	__prog_end
__prog_end:
	.byte	0
