; ASSIGN's resident part and the start of its transient part (spec D1-D3), in place of the shared
; runtime/crt0_com.s. Linked first (.text.start) at 100h by link_com.ld, so every symbol is the
; offset the specification gives it; `.org` stops the build if the hook outgrows 303h.
;
; 100h-121h  the jump to the start and the data at the original's offsets (spec Layout)
; 122h       the INT 21h hook (spec "The INT 21h hook"); its first 8 bytes are the signature
;            by which a later run finds this copy (lib.rs `run`)
;            the INT 25h/26h hooks, the lookups, the identity test
; 303h       assign_start: the transient part's start (SP, .bss, main), freed by function 31h
;
; Memory operands with CS: are the resident copy's own data, whatever DS the caller has.

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	.byte	0xe9				; 100h: JMP assign_start (disp 0200h: 102h holds 02h)
	.word	assign_start - (_start + 3)
	.globl	assign_table
assign_table:
	.byte	1, 2, 3, 4, 5, 6, 7, 8		; 103h: drive d (1 = A) goes to the byte at 102h + d
assign_current:
	.byte	0				; 10Bh: the logical default drive, 1 = A
assign_saved:
	.byte	0				; 10Ch: the caller's drive byte or DL
assign_saved_at:
	.word	0				; 10Dh: where that byte is (caller's DS)
	.globl	assign_old21
assign_old21:
	.word	0, 0				; 10Fh: the old INT 21h handler (offset, segment)
	.globl	assign_old25
assign_old25:
	.word	0, 0				; 113h
	.globl	assign_old26
assign_old26:
	.word	0, 0				; 117h
assign_caller:
	.word	0, 0				; 11Bh: the caller's IP and CS
assign_function:
	.byte	0				; 11Fh: AH of the call
	.globl	assign_equipment_word
assign_equipment_word:
	.word	0				; 120h: INT 11h at the start

	.org	0x22
	.globl	assign_int21
assign_int21:					; 122h
	cmp	ah, 0x0c
	jbe	.Ljump
	cmp	ah, 0x50
	je	.Ljump
	cmp	ah, 0x51
	je	.Ljump
	call	assign_identity
	je	.Ljump
	cmp	ah, 0x4b
	jne	.Lhook
	push	bx				; EXEC: translate the path, no return through here
	mov	bx, dx
	call	assign_letter
	pop	bx
.Ljump:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[assign_old21]
	.word	assign_old21

.Lhook:
	pop	word ptr cs:[assign_caller]
	pop	word ptr cs:[assign_caller + 2]
	push	ax
	push	bx
	cmp	ah, 0x0e
	jne	.Lclassify
	mov	al, dl				; select: the user's drive is the logical one
	inc	al
	mov	byte ptr cs:[assign_current], al
	call	assign_lookup
	dec	al
	mov	dl, al
	jmp	.Lcall
.Lclassify:
	call	assign_kind
	cmp	al, 1
	je	.Lpath
	cmp	al, 2
	je	.Lfcb
	cmp	al, 3
	jne	.Lcall
	mov	al, dl				; a drive in DL, 0 = default
	mov	byte ptr cs:[assign_saved], al
	or	al, al
	je	.Lcall
	call	assign_lookup
	mov	dl, al
	jmp	.Lcall
.Lpath:
	mov	bx, dx
	mov	al, byte ptr [bx]
	mov	byte ptr cs:[assign_saved], al
	mov	word ptr cs:[assign_saved_at], bx
	call	assign_letter
	jmp	.Lcall
.Lfcb:
	mov	bx, dx
	cmp	byte ptr [bx], 0xff
	jne	.Lnormal
	add	bx, 7				; an extended FCB: its drive byte
.Lnormal:
	mov	al, byte ptr [bx]
	or	al, al
	jne	.Ldrive
	mov	al, byte ptr cs:[assign_current]
.Ldrive:
	mov	byte ptr cs:[assign_saved], al
	mov	word ptr cs:[assign_saved_at], bx
	call	assign_lookup
	mov	byte ptr [bx], al
.Lcall:
	pop	bx
	pop	ax
	mov	byte ptr cs:[assign_function], ah
	.byte	0x2e, 0xff, 0x1e		; CALL FAR CS:[assign_old21]; its IRET takes the caller's flags
	.word	assign_old21
	pushf
	push	bx
	push	ax
	mov	ah, byte ptr cs:[assign_function]
	cmp	ah, 0x0e
	jne	.Lnot_select
	mov	dl, byte ptr cs:[assign_current]
	dec	dl
	jmp	.Lreturn
.Lnot_select:
	cmp	ah, 0x19
	jne	.Lnot_current
	pop	ax				; the logical default drive
	mov	al, byte ptr cs:[assign_current]
	dec	al
	jmp	.Lreturn_ax
.Lnot_current:
	cmp	ah, 0x32
	je	.Lrestore_dl
	cmp	ah, 0x47
	jne	.Lrestore_byte
.Lrestore_dl:
	mov	dl, byte ptr cs:[assign_saved]
	jmp	.Lreturn
.Lrestore_byte:
	call	assign_kind
	cmp	al, 1
	jb	.Lreturn
	cmp	al, 2
	ja	.Lreturn
	mov	bx, word ptr cs:[assign_saved_at]
	mov	al, byte ptr cs:[assign_saved]
	mov	byte ptr [bx], al
.Lreturn:
	pop	ax
.Lreturn_ax:
	pop	bx
	popf
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[assign_caller]
	.word	assign_caller

; AL = what the call in AH takes: 1 a path at DS:DX, 2 an FCB at DS:DX, 3 a drive in DL, 0 none.
assign_kind:
	mov	al, 3
	cmp	ah, 0x32
	je	.Lkind
	cmp	ah, 0x36
	je	.Lkind
	cmp	ah, 0x47
	je	.Lkind
	mov	al, 1
	cmp	ah, 0x41
	je	.Lkind
	cmp	ah, 0x43
	je	.Lkind
	cmp	ah, 0x4b
	je	.Lkind
	cmp	ah, 0x4e
	je	.Lkind
	cmp	ah, 0x56
	je	.Lkind
	cmp	ah, 0x39
	jb	.Lnot_path
	cmp	ah, 0x3d
	jbe	.Lkind
.Lnot_path:
	mov	al, 2
	cmp	ah, 0x25
	je	.Lnone
	cmp	ah, 0x26
	je	.Lnone
	cmp	ah, 0x0f
	jb	.Lnone
	cmp	ah, 0x17
	jbe	.Lkind
	cmp	ah, 0x21
	jb	.Lnone
	cmp	ah, 0x28
	jbe	.Lkind
.Lnone:
	mov	al, 0
.Lkind:
	ret

; The path at DS:BX: if its second byte is `:`, its letter becomes the letter of the drive it goes
; to. AX is kept.
assign_letter:
	cmp	byte ptr [bx + 1], 0x3a
	jne	.Lletter_done
	push	ax
	mov	al, byte ptr [bx]
	or	al, 0x20
	sub	al, 0x60
	call	assign_lookup
	add	al, 0x40
	mov	byte ptr [bx], al
	pop	ax
.Lletter_done:
	ret

; AL = the byte at CS:102h + AL (AL a signed index; 1 = A). AH and BX are kept.
assign_lookup:
	push	bx
	push	ax
	cbw
	xchg	bx, ax
	pop	ax
	mov	al, byte ptr cs:[bx + assign_table - 1]
	pop	bx
	ret

; AL (0 = A) = the drive it goes to, 0-based.
assign_lookup0:
	inc	al
	call	assign_lookup
	dec	al
	ret

	.globl	assign_int25
assign_int25:
	call	assign_lookup0
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[assign_old25]
	.word	assign_old25

	.globl	assign_int26
assign_int26:
	call	assign_lookup0
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[assign_old26]
	.word	assign_old26

; ZF = 1 if the table is the identity (103h-10Ah hold 1-8).
assign_identity:
	cmp	word ptr cs:[assign_table], 0x0201
	jne	.Lidentity_done
	cmp	word ptr cs:[assign_table + 2], 0x0403
	jne	.Lidentity_done
	cmp	word ptr cs:[assign_table + 4], 0x0605
	jne	.Lidentity_done
	cmp	word ptr cs:[assign_table + 6], 0x0807
.Lidentity_done:
	ret

; The transient part. DOS jumped to 100h with CS = DS = ES = SS = the PSP's segment. As
; crt0_com.s: SP at the end of the program's memory (at most 0FFF0h), .bss cleared, main called,
; INT 20h.
	.org	0x203
assign_start:					; 303h
	cld
	mov	ax, word ptr [2]
	mov	dx, cs
	sub	ax, dx
	cmp	ax, 0x0fff
	jb	.Lsized
	mov	ax, 0x0fff
.Lsized:
	mov	cl, 4
	shl	ax, cl
	mov	sp, ax
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	call	main
	int	0x20

; u16 assign_int11(void): INT 11h, the BIOS equipment word.
	.section	.text.assign_int11,"ax",@progbits
	.globl	assign_int11
assign_int11:
	int	0x11
	ret
