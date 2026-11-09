	.section	.text.hal,"ax",@progbits
	.globl	hal_putc
hal_putc:
	mov	dl, al
	mov	ah, 2
	int	0x21
	ret
	.globl	hal_exit
hal_exit:
	mov	ax, 0x4c00
	int	0x21
	.globl	hal_alloc64k
hal_alloc64k:
	push	bx
	mov	ax, cs
	mov	es, ax
	mov	bx, 0x1000
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
	ret
