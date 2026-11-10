; Far (retf) runtime pieces for medium-model programs (llvm-i8086/spec/abi.md D10). Every
; routine is called with `call seg:off` (Rust declares them `extern "C"`; ir_medium.py makes
; the calls `cc 1000`). Arguments as abi.md D3 (AX, DX, CX); BX, SI, DI, BP, DS and ES are
; preserved; the direction flag is clear.
;
;   struct Regs { u16 ax, bx, cx, dx, si, di, es, ds, flags; }   (offsets 0, 2, ... 16)

; far_int21(Regs *r), far_int10(Regs *r), far_int16(Regs *r): load the registers from *r
; (DS from r->ds, ES from r->es), execute the interrupt, store them back with the flags.
	.macro	FARINT	name, n
	.section	.text.\name,"ax",@progbits
	.globl	\name
\name:
	push	bp
	push	bx
	push	si
	push	di
	push	ds
	push	es
	mov	bp, ax
	push	bp
	mov	cx, word ptr [bp + 4]
	mov	dx, word ptr [bp + 6]
	mov	si, word ptr [bp + 8]
	mov	di, word ptr [bp + 10]
	mov	es, word ptr [bp + 12]
	mov	bx, word ptr [bp + 2]
	mov	ax, word ptr [bp + 14]
	mov	ds, ax
	mov	ax, word ptr [bp]
	int	\n
	pushf
	push	ax
	mov	bp, sp
	mov	bp, word ptr ss:[bp + 4]
	mov	word ptr ss:[bp + 2], bx
	mov	word ptr ss:[bp + 4], cx
	mov	word ptr ss:[bp + 6], dx
	mov	word ptr ss:[bp + 8], si
	mov	word ptr ss:[bp + 10], di
	mov	word ptr ss:[bp + 12], es
	mov	word ptr ss:[bp + 14], ds
	pop	ax
	mov	word ptr ss:[bp], ax
	pop	ax
	mov	word ptr ss:[bp + 16], ax
	add	sp, 2
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	pop	bp
	cld
	retf
	.endm

	FARINT	far_int21, 0x21
	FARINT	far_int10, 0x10
	FARINT	far_int16, 0x16

; far_exit(u16 code) -> !: INT 21h function 4Ch.
	.section	.text.far_exit,"ax",@progbits
	.globl	far_exit
far_exit:
	mov	ah, 0x4c
	int	0x21

; u16 far_shrink(void): gives DOS back the memory above the program (function 4Ah on the
; PSP's block, keeping up to __prog_end); 0 or the DOS error.
	.section	.text.far_shrink,"ax",@progbits
	.globl	far_shrink
far_shrink:
	push	bx
	push	es
	mov	es, word ptr [__psp]
	mov	bx, __prog_end@seg
	mov	ax, __prog_end
	add	ax, 15
	mov	cl, 4
	shr	ax, cl
	add	bx, ax
	sub	bx, word ptr [__psp]
	mov	ah, 0x4a
	int	0x21
	jc	.Lshrink_err
	xor	ax, ax
.Lshrink_err:
	pop	es
	pop	bx
	retf

; u16 far_alloc(u16 paragraphs): function 48h; the segment, or 0. With paragraphs = FFFFh the
; result is 0 and far_largest() gives what DOS offered.
	.section	.text.far_alloc,"ax",@progbits
	.globl	far_alloc
far_alloc:
	push	bx
	mov	bx, ax
	mov	ah, 0x48
	int	0x21
	jnc	.Lalloc_ok
	mov	word ptr [__far_largest], bx
	xor	ax, ax
.Lalloc_ok:
	pop	bx
	retf

; u16 far_largest(void): BX of the last failed far_alloc (the largest block DOS has).
	.section	.text.far_largest,"ax",@progbits
	.globl	far_largest
far_largest:
	mov	ax, word ptr [__far_largest]
	retf

	.section	.bss.__far_largest,"aw",@nobits
__far_largest:
	.zero	2

; u16 far_psp(void): the PSP's segment.
	.section	.text.far_psp,"ax",@progbits
	.globl	far_psp
far_psp:
	mov	ax, word ptr [__psp]
	retf

; far_copy(u16 dseg, u16 doff, u16 sseg, [stack] u16 soff, u16 n): n bytes from sseg:soff to
; dseg:doff, forward (rep movsb), any segments.
	.section	.text.far_copy,"ax",@progbits
	.globl	far_copy
far_copy:
	push	bp
	mov	bp, sp
	push	si
	push	di
	push	ds
	push	es
	mov	es, ax
	mov	di, dx
	mov	ds, cx
	mov	si, word ptr ss:[bp + 6]
	mov	cx, word ptr ss:[bp + 8]
	rep
	movsb
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bp
	retf
