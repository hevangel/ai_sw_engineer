; A BIOS-free stand-in for the platform's hal.s, for running Rust smoke
; programs on the bare 8088 core in tests: hal_putc appends the byte to a
; buffer in .bss instead of calling the BIOS, hal_halt executes hlt.

	.section	.text.hal_putc,"ax",@progbits
	.globl	hal_putc
hal_putc:
	push	bx
	mov	bx, word ptr [console_len]
	mov	byte ptr [bx+console], al
	inc	word ptr [console_len]
	pop	bx
	ret

	.section	.text.hal_halt,"ax",@progbits
	.globl	hal_halt
hal_halt:
	cli
	hlt
	jmp	hal_halt

	.section	.bss.console,"aw",@nobits
	.globl	console
console:
	.zero	2048

	.section	.bss.console_len,"aw",@nobits
	.globl	console_len
	.p2align	1
console_len:
	.zero	2
