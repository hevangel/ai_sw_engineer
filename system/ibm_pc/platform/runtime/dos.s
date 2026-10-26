; PC DOS function calls for the Rust crate ibm_pc_dos (../dos).
; Arguments follow llvm-i8086/spec/abi.md D3; BX, SI, DI and BP are preserved,
; and so are DS and ES.

; void dos_call(struct Regs *r)
;   struct Regs { u16 ax, bx, cx, dx, si, di, es, ds, flags; }  (offsets 0, 2 ... 16)
; Loads the registers from *r, executes INT 21h and stores them back. DS is
; loaded from r->ds for the call and restored afterwards; the stack is
; addressed through BP (SS), so DS can be anything while DOS runs.
	.section	.text.dos_call,"ax",@progbits
	.globl	dos_call
dos_call:
	push	bp
	push	bx
	push	si
	push	di
	push	ds
	push	es
	mov	bp, ax			; the Regs block
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
	int	0x21
	pushf
	push	ax
	mov	bp, sp
	mov	bp, word ptr [bp + 4]	; the Regs block again
	mov	word ptr [bp + 2], bx
	mov	word ptr [bp + 4], cx
	mov	word ptr [bp + 6], dx
	mov	word ptr [bp + 8], si
	mov	word ptr [bp + 10], di
	mov	word ptr [bp + 12], es
	mov	word ptr [bp + 14], ds
	pop	ax
	mov	word ptr [bp], ax
	pop	ax			; the flags
	mov	word ptr [bp + 16], ax
	add	sp, 2			; the saved block pointer
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	pop	bp
	ret

; void dos_exit(void) -> !: INT 20h
	.section	.text.dos_exit,"ax",@progbits
	.globl	dos_exit
dos_exit:
	int	0x20
