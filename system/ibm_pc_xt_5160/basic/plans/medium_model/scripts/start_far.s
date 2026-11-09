; Medium-model start (.EXE): DOS enters with DS = ES = the PSP, CS:IP and SS:SP from the header
; (i8086-ld --exe: CS = SS = the PSP's segment, offsets from 100h, SP = __stack_top).
	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	cld
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	call	main@seg:main
	mov	ax, 0x4c00
	int	0x21

	.section	.bss.stack,"aw",@nobits
	.zero	4096
	.globl	__stack_top
__stack_top:

	.section	.endmark,"aw",@progbits
	.globl	__prog_end
__prog_end:
	.byte	0
