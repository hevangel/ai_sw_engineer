; The BIOS calls and the compare DISKCOMP makes (spec D2). Arguments and results follow
; llvm-i8086/spec/abi.md D3; BX, SI, DI and BP are preserved, and so are DS and ES.

; u16 diskcomp_int13(struct Io *io)
;   struct Io { u16 ax, cx, dx, es; }  (offsets 0, 2, 4, 6)
; INT 13h with those registers and BX = 0; returns 0, or 100h + AH (the status) if the carry
; was set.
	.section	.text.diskcomp_int13,"ax",@progbits
	.globl	diskcomp_int13
diskcomp_int13:
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
	mov	ax, word ptr [si]
	xor	bx, bx
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

; u16 diskcomp_int11(void): INT 11h, the equipment word.
	.section	.text.diskcomp_int11,"ax",@progbits
	.globl	diskcomp_int11
diskcomp_int11:
	int	0x11
	ret

; u16 diskcomp_compare(u16 si, u16 es, u16 cx)
; REPZ CMPSB of CX bytes, DS:SI against ES:0000 (4B9h); 0 if all are equal, else CX after the
; first byte that differs, plus 1.
	.section	.text.diskcomp_compare,"ax",@progbits
	.globl	diskcomp_compare
diskcomp_compare:
	push	si
	push	di
	push	es
	mov	si, ax
	mov	es, dx
	xor	di, di
	xor	ax, ax
	jcxz	.Lsame
	cld
	rep
	cmpsb
	je	.Lsame
	mov	ax, cx
	inc	ax
.Lsame:
	pop	es
	pop	di
	pop	si
	ret
