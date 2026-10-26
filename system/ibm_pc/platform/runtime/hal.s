; Machine access for the Rust HAL (../hal): what has no Rust expression.
; Arguments follow llvm-i8086/spec/abi.md D3 (AX, DX, CX); BX, SI, DI and BP
; are preserved, and so is ES.

; void hal_putc(u8 c): the BIOS teletype (INT 10h, AH = 0Eh), page 0, grey.
	.section	.text.hal_putc,"ax",@progbits
	.globl	hal_putc
hal_putc:
	push	bx
	push	si
	push	di
	push	bp
	mov	ah, 0x0e
	mov	bx, 0x0007
	int	0x10
	pop	bp
	pop	di
	pop	si
	pop	bx
	ret

; void hal_halt(void) -> !
	.section	.text.hal_halt,"ax",@progbits
	.globl	hal_halt
hal_halt:
	cli
.Lhalt:
	hlt
	jmp	.Lhalt

; void hal_outb(u16 port, u8 value)
	.section	.text.hal_outb,"ax",@progbits
	.globl	hal_outb
hal_outb:
	xchg	ax, dx			; ax = value, dx = port
	out	dx, al
	ret

; u8 hal_inb(u16 port)
	.section	.text.hal_inb,"ax",@progbits
	.globl	hal_inb
hal_inb:
	mov	dx, ax
	in	al, dx
	xor	ah, ah
	ret

; u16 hal_far_read16(u16 segment, u16 offset)
	.section	.text.hal_far_read16,"ax",@progbits
	.globl	hal_far_read16
hal_far_read16:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	ax, word ptr es:[bx]
	pop	es
	pop	bx
	ret

; void hal_far_write16(u16 segment, u16 offset, u16 value)
	.section	.text.hal_far_write16,"ax",@progbits
	.globl	hal_far_write16
hal_far_write16:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	word ptr es:[bx], cx
	pop	es
	pop	bx
	ret
