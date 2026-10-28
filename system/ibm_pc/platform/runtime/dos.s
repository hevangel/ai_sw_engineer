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

; u16 dos_cs(void): the code segment, which is also DS, ES and SS in a .COM program
	.section	.text.dos_cs,"ax",@progbits
	.globl	dos_cs
dos_cs:
	mov	ax, cs
	ret

; u16 dos_far_read16(u16 segment, u16 offset)
	.section	.text.dos_far_read16,"ax",@progbits
	.globl	dos_far_read16
dos_far_read16:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	ax, word ptr es:[bx]
	pop	es
	pop	bx
	ret

; void dos_far_write16(u16 segment, u16 offset, u16 value)
	.section	.text.dos_far_write16,"ax",@progbits
	.globl	dos_far_write16
dos_far_write16:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	word ptr es:[bx], cx
	pop	es
	pop	bx
	ret

; void dos_flush_guarded(u8 target, u8 original, u8 modified)
; PC DOS function 0Dh (disk reset: writes the buffers) with a critical-error
; handler that ignores write-protect errors on drive `target` unless the
; program changed the FAT (`modified` == 1); every other error goes to the
; handler that was installed (after selecting drive `original` again). The
; vector is restored afterwards. This is what the original CHKDSK.COM does
; at its address 385h.
	.section	.bss.dos_ce,"aw",@nobits
dos_ce_target:
	.zero	1
dos_ce_original:
	.zero	1
dos_ce_modified:
	.zero	1
dos_ce_old:
	.zero	4

	.section	.text.dos_flush_guarded,"ax",@progbits
	.globl	dos_flush_guarded
dos_flush_guarded:
	push	bx
	push	si
	push	di
	push	bp
	push	ds
	push	es
	mov	byte ptr [dos_ce_target], al
	mov	byte ptr [dos_ce_original], dl
	mov	byte ptr [dos_ce_modified], cl
	push	ds
	pop	es			; ES = this program's segment
	xor	ax, ax
	mov	ds, ax
	mov	si, 0x90		; the INT 24h vector
	mov	di, dos_ce_old
	mov	cx, 2
	cld
	rep
	movsw
	push	es
	pop	ds
	mov	dx, dos_critical_handler
	mov	ax, 0x2524
	int	0x21
	mov	ah, 0x0d
	int	0x21
	xor	ax, ax
	mov	es, ax
	mov	di, 0x90
	mov	si, dos_ce_old
	mov	cx, 2
	rep
	movsw
	pop	es
	pop	ds
	pop	bp
	pop	di
	pop	si
	pop	bx
	ret

dos_critical_handler:
	sti
	and	di, 0xff
	jne	.Lchain			; only the write-protect error
	cmp	ah, 3
	jne	.Lchain
	cmp	byte ptr cs:[dos_ce_target], al
	jne	.Lchain
	cmp	byte ptr cs:[dos_ce_modified], 1
	je	.Lchain
	xor	al, al			; ignore
	iret
.Lchain:
	mov	si, ax
	push	dx
	mov	dl, byte ptr cs:[dos_ce_original]
	mov	ah, 0x0e
	int	0x21
	pop	dx
	mov	ax, si
	.byte	0x2e, 0xff, 0x2e	; jmp far cs:[dos_ce_old]
	.word	dos_ce_old
