; The BIOS calls DISKCOPY makes (spec D2). Arguments and results follow llvm-i8086/spec/abi.md
; D3; BX, SI, DI and BP are preserved, and so are DS and ES.

; u16 diskcopy_int13(struct Io *io)
;   struct Io { u16 ax, cx, dx, es, bx; }  (offsets 0, 2, 4, 6, 8)
; INT 13h with those registers; returns 0, or 100h + AH (the status) if the carry was set.
	.section	.text.diskcopy_int13,"ax",@progbits
	.globl	diskcopy_int13
diskcopy_int13:
	push	bx
	push	si
	push	di
	push	bp
	push	ds
	push	es
	mov	si, ax
	mov	cx, word ptr [si + 2]
	mov	dx, word ptr [si + 4]
	mov	es, word ptr [si + 6]
	mov	bx, word ptr [si + 8]
	mov	ax, word ptr [si]
	int	0x13
	jc	.Lerror
	xor	ax, ax
	jmp	.Lout
.Lerror:
	mov	al, ah
	mov	ah, 1
.Lout:
	pop	es
	pop	ds
	pop	bp
	pop	di
	pop	si
	pop	bx
	ret

; u16 diskcopy_int11(void): INT 11h, the equipment word.
	.section	.text.diskcopy_int11,"ax",@progbits
	.globl	diskcopy_int11
diskcopy_int11:
	int	0x11
	ret
