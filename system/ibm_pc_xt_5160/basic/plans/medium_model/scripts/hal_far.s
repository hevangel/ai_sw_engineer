; Far HAL for the medium model: every routine returns with retf (abi.md D8).
	.section	.text.hal,"ax",@progbits
	.globl	hal_putc
hal_putc:
	mov	dl, al
	mov	ah, 2
	int	0x21
	retf
	.globl	hal_exit
hal_exit:
	mov	ax, 0x4c00
	int	0x21
; Shrink the program's block to what it occupies, then allocate a 64 KiB segment (AX = it,
; or 0). ES is DS again on return (the compiler keeps ES = DS).
	.globl	hal_alloc64k
hal_alloc64k:
	push	bx
	mov	ax, cs
	mov	es, ax
	mov	bx, __prog_end@seg
	mov	ax, __prog_end
	add	ax, 15
	mov	cl, 4
	shr	ax, cl
	add	bx, ax
	mov	ax, cs
	sub	bx, ax
	mov	ah, 0x4a
	int	0x21
	mov	bx, 0x1000
	mov	ah, 0x48
	int	0x21
	jnc	.Lok
	xor	ax, ax
.Lok:
	push	ds
	pop	es
	pop	bx
	retf
