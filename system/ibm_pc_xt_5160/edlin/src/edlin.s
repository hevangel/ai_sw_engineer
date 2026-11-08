; The machine side of the EDLIN core (src/main.rs, spec D2): INT 21h with Ctrl-C caught, the
; INT 23h and INT 24h handlers, INT 20h, and the string instructions on the program segment
; prefix's segment (edlin_seg, start.s), where the core keeps the original's data and text.
; Arguments and results follow llvm-i8086/spec/abi.md D3 (AX, DX, CX; AX back); BX, SI, DI,
; BP, DS and ES are preserved and the direction flag is clear on return.

	.section	.bss.edlin_state,"aw",@nobits
; SP inside edlin_dos while INT 21h runs: where edlin_int23 resumes
edlin_catch:
	.zero	2
; the INT 24h vector found by edlin_hook24 (offset, segment)
edlin_old24:
	.zero	4

	.section	.data.edlin_ret24,"aw",@progbits
; where the old INT 24h handler's IRET comes back to (the original's INT_24_RETADDR, DF3h)
edlin_ret24:
	.word	edlin_int24_back

; u16 edlin_dos(struct R *r)
;   struct R { u16 ax, bx, cx, dx, si, di, ds, flags; }  (offsets 0, 2 ... 14)
; INT 21h with AX, BX, CX, DX, SI, DI from *r, DS = r->ds and ES = edlin_seg; the registers,
; DS and the flags DOS returns stored back. Returns 0, or 1 if Ctrl-C ended the call: DOS then
; ran INT 23h, which is edlin_int23 once edlin_set23 installed it, and *r is left as it was.
	.section	.text.edlin_dos,"ax",@progbits
	.globl	edlin_dos
edlin_dos:
	push	bp
	push	bx
	push	si
	push	di
	push	ds
	push	es
	push	ax			; the block
	mov	word ptr [edlin_catch], sp
	mov	bp, ax
	mov	ax, word ptr [edlin_seg]
	mov	es, ax
	mov	bx, word ptr [bp + 2]
	mov	cx, word ptr [bp + 4]
	mov	dx, word ptr [bp + 6]
	mov	si, word ptr [bp + 8]
	mov	di, word ptr [bp + 10]
	mov	ax, word ptr [bp + 12]
	mov	ds, ax
	mov	ax, word ptr [bp]
	int	0x21
	pushf
	push	ax
	mov	bp, sp
	mov	bp, word ptr [bp + 4]	; the block again (SS is the program's)
	mov	word ptr [bp + 2], bx
	mov	word ptr [bp + 4], cx
	mov	word ptr [bp + 6], dx
	mov	word ptr [bp + 8], si
	mov	word ptr [bp + 10], di
	mov	word ptr [bp + 12], ds
	pop	ax
	mov	word ptr [bp], ax
	pop	ax
	mov	word ptr [bp + 14], ax
	xor	ax, ax
	.globl	edlin_dos_out
edlin_dos_out:
	add	sp, 2
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	pop	bp
	ret

; INT 23h (Ctrl-C), as the original's ABORTCOM/ABORTINS/ABORTMERGE begin (D5Ah, CB8h): the
; program's segments and stack, interrupts on; then not back to DOS but out of the edlin_dos
; call that was running, with 1. The core runs the handler the original has installed.
	.section	.text.edlin_int23,"ax",@progbits
	.globl	edlin_int23
edlin_int23:
	cli
	mov	ax, cs
	mov	ds, ax
	mov	es, ax
	mov	ss, ax
	mov	sp, word ptr [edlin_catch]
	sti
	cld
	mov	ax, 1
	jmp	edlin_dos_out

; INT 24h, the original's INT_24 (DF5h): the old handler is run with a frame that returns to
; edlin_int24_back; its answer in AL is passed back to DOS, except Abort (2), which restores
; the directory (edlin_critical_abort in main.rs, on the program's stack) and ends EDLIN.
	.section	.text.edlin_int24,"ax",@progbits
	.globl	edlin_int24
edlin_int24:
	pushf
	push	cs
	push	word ptr cs:[edlin_ret24]
	push	word ptr cs:[edlin_old24 + 2]
	push	word ptr cs:[edlin_old24]
	retf
edlin_int24_back:
	cmp	al, 2
	jne	.Liret
	cli
	mov	ax, cs
	mov	ds, ax
	mov	es, ax
	mov	ss, ax
	call	edlin_stack_top
	mov	sp, ax
	sti
	cld
	call	edlin_critical_abort
.Liret:
	iret

; void edlin_set23(void): AX=2523h, INT 23h to edlin_int23.
	.section	.text.edlin_set23,"ax",@progbits
	.globl	edlin_set23
edlin_set23:
	mov	dx, edlin_int23
	mov	ax, 0x2523
	int	0x21
	ret

; void edlin_hook24(void): AX=3524h (the old vector kept), AX=2524h to edlin_int24.
	.section	.text.edlin_hook24,"ax",@progbits
	.globl	edlin_hook24
edlin_hook24:
	push	bx
	push	es
	mov	ax, 0x3524
	int	0x21
	mov	word ptr [edlin_old24], bx
	mov	word ptr [edlin_old24 + 2], es
	mov	dx, edlin_int24
	mov	ax, 0x2524
	int	0x21
	pop	es
	pop	bx
	ret

; void edlin_unhook24(void): AX=2524h with the kept vector (the original's LDS DX,[HARDCH]).
	.section	.text.edlin_unhook24,"ax",@progbits
	.globl	edlin_unhook24
edlin_unhook24:
	push	ds
	mov	dx, word ptr [edlin_old24]
	mov	ax, word ptr [edlin_old24 + 2]
	mov	ds, ax
	mov	ax, 0x2524
	int	0x21
	pop	ds
	ret

; void edlin_exit(void) -> !: INT 20h with CS = the PSP's segment (a far jump to PSP:0000,
; which holds INT 20h).
	.section	.text.edlin_exit,"ax",@progbits
	.globl	edlin_exit
edlin_exit:
	push	word ptr [edlin_seg]
	xor	ax, ax
	push	ax
	retf

; u16 edlin_peek(u16 offset): the byte at edlin_seg:offset.
	.section	.text.edlin_peek,"ax",@progbits
	.globl	edlin_peek
edlin_peek:
	push	bx
	push	es
	mov	bx, ax
	mov	ax, word ptr [edlin_seg]
	mov	es, ax
	mov	al, byte ptr es:[bx]
	xor	ah, ah
	pop	es
	pop	bx
	ret

; void edlin_poke(u16 offset, u16 value): the byte at edlin_seg:offset.
	.section	.text.edlin_poke,"ax",@progbits
	.globl	edlin_poke
edlin_poke:
	push	bx
	push	es
	mov	bx, ax
	mov	ax, word ptr [edlin_seg]
	mov	es, ax
	mov	byte ptr es:[bx], dl
	pop	es
	pop	bx
	ret

; u16 edlin_far_byte(u16 segment, u16 offset)
	.section	.text.edlin_far_byte,"ax",@progbits
	.globl	edlin_far_byte
edlin_far_byte:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	al, byte ptr es:[bx]
	xor	ah, ah
	pop	es
	pop	bx
	ret

; void edlin_movsb(struct S *s)   struct S { u16 di, si, cx, down; }
; REP MOVSB in edlin_seg (STD if down); DI and SI after it stored back.
	.section	.text.edlin_movsb,"ax",@progbits
	.globl	edlin_movsb
edlin_movsb:
	push	bx
	push	si
	push	di
	push	ds
	push	es
	mov	bx, ax
	mov	di, word ptr [bx]
	mov	si, word ptr [bx + 2]
	mov	cx, word ptr [bx + 4]
	mov	dx, word ptr [bx + 6]
	mov	ax, word ptr [edlin_seg]
	mov	es, ax
	mov	ds, ax
	or	dx, dx
	jz	.Lmovup
	std
.Lmovup:
	rep
	movsb
	cld
	mov	ax, ss
	mov	ds, ax
	mov	word ptr [bx], di
	mov	word ptr [bx + 2], si
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	ret

; u16 edlin_scasb(struct S *s)   struct S { u16 di, cx, al, down; }
; REPNE SCASB in edlin_seg (CX not 0; STD if down); DI and CX stored back; 1 if ZF.
	.section	.text.edlin_scasb,"ax",@progbits
	.globl	edlin_scasb
edlin_scasb:
	push	bx
	push	di
	push	es
	mov	bx, ax
	mov	di, word ptr [bx]
	mov	cx, word ptr [bx + 2]
	mov	dx, word ptr [bx + 6]
	mov	ax, word ptr [edlin_seg]
	mov	es, ax
	mov	al, byte ptr [bx + 4]
	or	dx, dx
	jz	.Lscanup
	std
.Lscanup:
	repne
	scasb
	cld
	mov	word ptr [bx], di
	mov	word ptr [bx + 2], cx
	mov	ax, 0
	jne	.Lscanout
	inc	ax
.Lscanout:
	pop	es
	pop	di
	pop	bx
	ret

; u16 edlin_cmpsb(struct S *s)   struct S { u16 si, di, cx; }
; REPE CMPSB of DS:SI and ES:DI, both in edlin_seg (CX not 0); SI, DI, CX stored back; 1 if ZF.
	.section	.text.edlin_cmpsb,"ax",@progbits
	.globl	edlin_cmpsb
edlin_cmpsb:
	push	bx
	push	si
	push	di
	push	ds
	push	es
	mov	bx, ax
	mov	si, word ptr [bx]
	mov	di, word ptr [bx + 2]
	mov	cx, word ptr [bx + 4]
	mov	ax, word ptr [edlin_seg]
	mov	es, ax
	mov	ds, ax
	repe
	cmpsb
	mov	ax, ss
	mov	ds, ax
	mov	word ptr [bx], si
	mov	word ptr [bx + 2], di
	mov	word ptr [bx + 4], cx
	mov	ax, 0
	jne	.Lcmpout
	inc	ax
.Lcmpout:
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	ret
