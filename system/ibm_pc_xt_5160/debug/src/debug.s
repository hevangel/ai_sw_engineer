; The machine side of the DEBUG core (src/main.rs): INT 21h/10h calls, far memory on arbitrary
; segments, function 26h to build the user segment, and the trace/go engine (the original's
; DEXIT/REENTER) that runs the debugged program with single-step (INT 1) and breakpoints (INT 3)
; and catches its termination (INT 22h). Arguments follow llvm-i8086/spec/abi.md D3 (AX, DX, CX;
; AX back); BX, SI, DI, BP, DS, ES are preserved and DF is clear on return.

	.set	UR_AX, 0
	.set	UR_BX, 2
	.set	UR_CX, 4
	.set	UR_DX, 6
	.set	UR_SP, 8
	.set	UR_BP, 10
	.set	UR_SI, 12
	.set	UR_DI, 14
	.set	UR_DS, 16
	.set	UR_ES, 18
	.set	UR_SS, 20
	.set	UR_CS, 22
	.set	UR_IP, 24
	.set	UR_F, 26

; --- state (in the image's segment, addressed CS-relative from the handlers) ---
	.section	.data.debug_state,"aw",@progbits
debug_regs:	.word 0
debug_bps:	.word 0
debug_nbp:	.word 0
debug_ss:	.word 0
debug_sp:	.word 0
debug_bp:	.word 0
debug_result:	.word 0
debug_tmp:	.word 0
debug_bpsave:	.zero 10

; u16 debug_out(u16 ch): INT 21h AH=02h with DL = ch & 7Fh.
	.section	.text.debug_out,"ax",@progbits
	.globl	debug_out
debug_out:
	push	bx
	mov	dl, al
	and	dl, 0x7f
	mov	ah, 2
	int	0x21
	pop	bx
	ret

; u16 debug_line(u16 buf_off): INT 21h AH=0Ah into DG:buf (buf in DG at buf_off, a 162-byte
; area); returns the count. The caller reads the bytes with debug_peek(DG, off).
	.section	.text.debug_line,"ax",@progbits
	.globl	debug_line
debug_line:
	push	bx
	push	ds
	mov	bx, ax			; of the buffer in DG
	mov	ax, word ptr cs:[debug_dg]
	mov	ds, ax
	mov	byte ptr [bx], 160	; max length
	mov	dx, bx
	mov	ah, 0x0a
	int	0x21
	mov	bl, byte ptr [bx + 1]	; count
	xor	bh, bh
	mov	ax, bx
	pop	ds
	pop	bx
	ret

; u16 debug_columns(void): INT 10h AH=0Fh returns AH = columns.
	.section	.text.debug_columns,"ax",@progbits
	.globl	debug_columns
debug_columns:
	push	bx
	mov	ah, 0x0f
	int	0x10
	mov	al, ah
	xor	ah, ah
	pop	bx
	ret

; void debug_create_psb(u16 seg): INT 21h AH=26h with DX = seg.
	.section	.text.debug_create_psb,"ax",@progbits
	.globl	debug_create_psb
debug_create_psb:
	mov	dx, ax
	mov	ah, 0x26
	int	0x21
	ret

; void debug_set_terminate(u16 handler_unused): INT 21h AX=2522h, DX=debug_terminate.
	.section	.text.debug_set_terminate,"ax",@progbits
	.globl	debug_set_terminate
debug_set_terminate:
	push	ds
	mov	ax, cs
	mov	ds, ax
	mov	dx, debug_terminate
	mov	ax, 0x2522
	int	0x21
	pop	ds
	ret

; u16 debug_peek(u16 seg, u16 off): byte at seg:off.
	.section	.text.debug_peek,"ax",@progbits
	.globl	debug_peek
debug_peek:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	al, byte ptr es:[bx]
	xor	ah, ah
	pop	es
	pop	bx
	ret

; void debug_poke(u16 seg, u16 off, u16 val): CX = val.
	.section	.text.debug_poke,"ax",@progbits
	.globl	debug_poke
debug_poke:
	push	bx
	push	es
	mov	es, ax
	mov	bx, dx
	mov	al, cl
	mov	byte ptr es:[bx], al
	pop	es
	pop	bx
	ret

; u16 debug_general(struct R *r): INT 21h from a register block for file/exec/PDB calls.
;   struct R { u16 ax, bx, cx, dx, si, di, ds, es, flags }  (offsets 0,2,...,16)
	.section	.text.debug_general,"ax",@progbits
	.globl	debug_general
debug_general:
	push	bp
	push	bx
	push	si
	push	di
	push	ds
	push	es
	push	ax
	mov	bp, ax
	mov	bx, word ptr [bp + 2]
	mov	cx, word ptr [bp + 4]
	mov	dx, word ptr [bp + 6]
	mov	si, word ptr [bp + 8]
	mov	di, word ptr [bp + 10]
	mov	es, word ptr [bp + 14]
	mov	ax, word ptr [bp + 12]
	mov	ds, ax
	mov	ax, word ptr [bp]
	int	0x21
	pushf
	push	ax
	mov	bp, sp
	mov	bp, word ptr [bp + 4]
	mov	word ptr [bp + 2], bx
	mov	word ptr [bp + 4], cx
	mov	word ptr [bp + 6], dx
	mov	word ptr [bp + 8], si
	mov	word ptr [bp + 10], di
	mov	word ptr [bp + 14], es
	mov	word ptr [bp + 12], ds
	pop	ax
	mov	word ptr [bp], ax
	pop	ax
	mov	word ptr [bp + 16], ax
	add	sp, 2
	pop	es
	pop	ds
	pop	di
	pop	si
	pop	bx
	pop	bp
	ret

; void debug_exit(void) -> !: INT 20h with CS = the PSP's segment.
	.section	.text.debug_exit,"ax",@progbits
	.globl	debug_exit
debug_exit:
	push	word ptr cs:[debug_dg]
	xor	ax, ax
	push	ax
	retf

; ------------------------------------------------------------------------------------------------
; u16 debug_run(UserRegs *regs, (u16,u16)* bps, u16 nbp): run the user program; 0 if it stopped at
; a trace step or breakpoint (regs updated), 1 if it terminated. The trap flag in regs->f selects
; single-step. Models the original's DEXIT (load regs, set vectors, IRET into the program) and
; REENTER/BREAKFIX/TERMINATE (save regs, come back here).
	.section	.text.debug_run,"ax",@progbits
	.globl	debug_run
debug_run:
	push	bp
	push	bx
	push	si
	push	di
	mov	word ptr cs:[debug_regs], ax
	mov	word ptr cs:[debug_bps], dx
	mov	word ptr cs:[debug_nbp], cx
	; set breakpoints
	mov	si, dx
	mov	di, debug_bpsave
	jcxz	.Lbpdone
.Lbploop:
	push	cx
	mov	bx, word ptr cs:[si]
	mov	es, word ptr cs:[si + 2]
	mov	al, byte ptr es:[bx]
	mov	byte ptr cs:[di], al
	mov	byte ptr es:[bx], 0xcc
	add	si, 4
	inc	di
	pop	cx
	loop	.Lbploop
.Lbpdone:
	; install INT 1, INT 3, INT 22h, INT 23h vectors (CS = this image)
	push	ds
	xor	ax, ax
	mov	ds, ax
	mov	word ptr [4], debug_reenter
	mov	word ptr [6], cs
	mov	word ptr [0x0c], debug_breakfix
	mov	word ptr [0x0e], cs
	mov	word ptr [0x88], debug_terminate
	mov	word ptr [0x8a], cs
	mov	word ptr [0x8c], debug_terminate
	mov	word ptr [0x8e], cs
	pop	ds
	; set the current PDB to the user segment so INT 20h ends the program, not DEBUG
	mov	si, word ptr cs:[debug_regs]
	mov	bx, word ptr cs:[si + UR_DS]
	mov	ah, 0x50
	int	0x21
	; save DEBUG's return context
	mov	word ptr cs:[debug_ss], ss
	mov	word ptr cs:[debug_sp], sp
	mov	word ptr cs:[debug_bp], bp
	; build the IRET frame on the user stack and load the registers
	mov	si, word ptr cs:[debug_regs]
	cli
	mov	ss, word ptr cs:[si + UR_SS]
	mov	sp, word ptr cs:[si + UR_SP]
	mov	ax, word ptr cs:[si + UR_F]
	push	ax			; flags
	push	word ptr cs:[si + UR_CS]
	push	word ptr cs:[si + UR_IP]
	mov	ax, word ptr cs:[si + UR_AX]
	mov	bx, word ptr cs:[si + UR_BX]
	mov	cx, word ptr cs:[si + UR_CX]
	mov	dx, word ptr cs:[si + UR_DX]
	mov	bp, word ptr cs:[si + UR_BP]
	mov	di, word ptr cs:[si + UR_DI]
	mov	es, word ptr cs:[si + UR_ES]
	push	word ptr cs:[si + UR_DS]
	mov	si, word ptr cs:[si + UR_SI]
	pop	ds
	iret

debug_breakfix:
	push	bp
	mov	bp, sp
	dec	word ptr [bp + 2]	; IP in the frame: back over the CC
	pop	bp
	; fall into reenter
debug_reenter:
	mov	word ptr cs:[debug_tmp], si
	mov	si, word ptr cs:[debug_regs]
	mov	word ptr cs:[si + UR_AX], ax
	mov	word ptr cs:[si + UR_BX], bx
	mov	word ptr cs:[si + UR_CX], cx
	mov	word ptr cs:[si + UR_DX], dx
	mov	word ptr cs:[si + UR_BP], bp
	mov	word ptr cs:[si + UR_DI], di
	mov	ax, word ptr cs:[debug_tmp]
	mov	word ptr cs:[si + UR_SI], ax
	mov	word ptr cs:[si + UR_ES], es
	mov	word ptr cs:[si + UR_DS], ds
	mov	ax, ss
	mov	word ptr cs:[si + UR_SS], ax
	mov	ax, sp
	add	ax, 6
	mov	word ptr cs:[si + UR_SP], ax
	pop	word ptr cs:[si + UR_IP]
	pop	word ptr cs:[si + UR_CS]
	pop	ax
	and	ah, 0xfe		; clear the trap flag in the saved value
	mov	word ptr cs:[si + UR_F], ax
	mov	word ptr cs:[debug_result], 0
	jmp	debug_back

debug_terminate:
	mov	word ptr cs:[debug_result], 1
	jmp	debug_back

; common return: restore DEBUG's stack and the current PDB, clear breakpoints, return result
debug_back:
	cli
	mov	ax, word ptr cs:[debug_ss]
	mov	ss, ax
	mov	sp, word ptr cs:[debug_sp]
	mov	bp, word ptr cs:[debug_bp]
	mov	ax, cs
	mov	ds, ax
	mov	es, ax
	sti
	cld
	; restore the current PDB to DEBUG (DG)
	mov	bx, word ptr cs:[debug_dg]
	mov	ah, 0x50
	int	0x21
	; clear breakpoints
	mov	cx, word ptr cs:[debug_nbp]
	jcxz	.Lcleared
	mov	si, word ptr cs:[debug_bps]
	mov	di, debug_bpsave
.Lclear:
	push	cx
	mov	bx, word ptr cs:[si]
	mov	es, word ptr cs:[si + 2]
	mov	al, byte ptr cs:[di]
	mov	byte ptr es:[bx], al
	add	si, 4
	inc	di
	pop	cx
	loop	.Lclear
	mov	ax, cs
	mov	es, ax
.Lcleared:
	mov	ax, word ptr cs:[debug_result]
	pop	di
	pop	si
	pop	bx
	pop	bp
	ret
