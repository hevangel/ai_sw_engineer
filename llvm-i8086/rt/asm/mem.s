; memcpy, memmove, memset, memcmp and bcmp for llvm-i8086.
;
; Arguments follow spec/abi.md D3: (AX, DX, CX). BX, SI, DI and BP are
; preserved. The direction flag is clear on entry and on exit.

; void *memcpy(void *dst, const void *src, size_t n): returns dst in AX.
	.section	.text.memcpy,"ax",@progbits
	.globl	memcpy
	.type	memcpy,@function
memcpy:
	push	si
	push	di
	mov	di, ax
	mov	si, dx
	rep
	movsb
	pop	di
	pop	si
	ret
	.size	memcpy, .-memcpy

; void *memmove(void *dst, const void *src, size_t n): forward unless the
; destination starts inside the source, then backward from the end.
	.section	.text.memmove,"ax",@progbits
	.globl	memmove
	.type	memmove,@function
memmove:
	push	si
	push	di
	mov	di, ax
	mov	si, dx
	cmp	di, si
	jbe	.Lmemmove_forward		; dst <= src: forward is safe
	mov	dx, di
	sub	dx, si			; dst - src
	cmp	dx, cx
	jae	.Lmemmove_forward		; the ranges do not overlap
	; backward: start at the last byte
	mov	dx, cx
	dec	dx			; n - 1 (n = 0 is handled by rep)
	add	si, dx
	add	di, dx
	std
	rep
	movsb
	cld
	pop	di
	pop	si
	ret
.Lmemmove_forward:
	rep
	movsb
	pop	di
	pop	si
	ret
	.size	memmove, .-memmove

; void *memset(void *dst, int c, size_t n): returns dst in AX.
	.section	.text.memset,"ax",@progbits
	.globl	memset
	.type	memset,@function
memset:
	push	di
	mov	di, ax
	mov	al, dl
	mov	dx, di
	rep
	stosb
	mov	ax, dx
	pop	di
	ret
	.size	memset, .-memset

; int memcmp(const void *a, const void *b, size_t n): the difference of the
; first unequal bytes (unsigned), or 0.
	.section	.text.memcmp,"ax",@progbits
	.globl	memcmp
	.type	memcmp,@function
	.globl	bcmp
	.type	bcmp,@function
memcmp:
bcmp:
	push	si
	push	di
	mov	si, ax
	mov	di, dx
	xor	ax, ax			; equal so far (also for n = 0)
	jcxz	.Lmemcmp_done
	repe
	cmpsb
	je	.Lmemcmp_done
	mov	al, byte ptr [si-1]
	xor	ah, ah
	mov	dl, byte ptr [di-1]
	xor	dh, dh
	sub	ax, dx
.Lmemcmp_done:
	pop	di
	pop	si
	ret
	.size	memcmp, .-memcmp
