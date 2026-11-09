; GRAPHICS's resident INT 5 handler and the start of its transient part (spec D1-D3), in place of
; the shared runtime/crt0_com.s. Linked first (.text.start) at 100h by link_com.ld, so every
; symbol is the offset the specification gives it; `.org` stops the build if the handler
; outgrows 335h. This repository's own code, written from spec/spec.md "The INT 5 handler"; the
; logic is lib.rs `int5`.
;
; 100h-113h  the jump to the start (NOP NOP NOP once installed), STI, JMP 114h, the data
; 114h       the INT 5 handler and its helpers
; 335h       graphics_start: the transient part's start (SP, .bss, main); lib.rs `install`
;            moves 100h-335h to 60h and stays resident with INT 27h (DX = 296h)
;
; The handler runs in the moved copy, CS = the program's segment - 0Ah, at the same offsets.
; It keeps AX, BX, CX, DX and DS, and changes no other register. Inside it DS = CS.

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	.byte	0xe9				; 100h: JMP graphics_start
	.word	graphics_start - (_start + 3)
	sti					; 103h
	.byte	0xeb, graphics_int5 - 1f	; 104h: JMP SHORT 114h (EB 0E)
1:
	.globl	graphics_pixels
graphics_pixels:
	.byte	0, 0, 0, 0			; 106h-109h: a column's four pixels, 109h the top row
graphics_first:
	.byte	0				; 10Ah: the column's first printer byte
graphics_mode320:
	.byte	0				; 10Bh: A0h in 320 x 200, 0 in 640 x 200
	.globl	graphics_old5
graphics_old5:
	.word	0, 0				; 10Ch: the INT 5 vector found at the start
graphics_sp:
	.word	0				; 110h: SP for the error exit
graphics_count:
	.word	0				; 112h: the band's length

	.org	0x14
	.globl	graphics_int5
graphics_int5:					; 114h
	push	ax
	push	ds
	push	dx
	push	cx
	push	bx
	xor	dx, dx
	mov	ax, 0x40
	mov	ds, ax
	mov	al, byte ptr [0x49]		; the video mode
	mov	bl, 0xa0
	cmp	al, 4
	je	.Lgraphics
	cmp	al, 5
	je	.Lgraphics
	mov	bl, 0
	cmp	al, 6
	je	.Lgraphics
	pop	bx				; a text mode: the old handler
	pop	cx
	pop	dx
	pop	ds
	pop	ax
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[graphics_old5]
	.word	graphics_old5

.Lgraphics:
	mov	al, 1
	cmp	byte ptr [0x100], al		; 0040:0100h = 0050:0000h, the status byte
	je	.Lreturn			; a print screen is running
	mov	byte ptr [0x100], al
	mov	ax, cs
	mov	ds, ax
	mov	word ptr [graphics_sp], sp
	mov	byte ptr [graphics_mode320], bl
	call	new_line
	call	spacing_24
	cmp	byte ptr [graphics_mode320], 0
	je	.L640

; 320 x 200: DX = the band's top row
.Lband320:
	mov	cx, 319
.Lscan320:
	call	any_of_four
	jnz	.Lfound320
	jcxz	.Lnext320			; no pixel in the band: only the new line
	dec	cx
	jmp	.Lscan320
.Lfound320:
	inc	cx
	mov	word ptr [graphics_count], cx
	push	dx
	call	bit_image
	pop	dx
	xor	cx, cx
.Lcolumn320:
	push	dx
	mov	bx, graphics_pixels + 3
.Lread320:
	mov	ah, 0x0d
	int	0x10
	mov	byte ptr [bx], al
	dec	bx
	inc	dx
	cmp	bx, graphics_pixels - 1
	jne	.Lread320
	pop	dx
	xor	ah, ah				; the first byte: colour >= 2 (signed), 3 bits a pixel
	mov	bx, graphics_pixels + 3
.Lhigh320:
	shl	ah, 1
	shl	ah, 1
	cmp	byte ptr [bx], 2
	jl	1f
	or	ah, 3
1:
	dec	bx
	cmp	bx, graphics_pixels - 1
	jne	.Lhigh320
	mov	al, ah
	mov	byte ptr [graphics_first], al
	call	send
	xor	ah, ah				; the second byte: bit 0 of the colour
	mov	bx, graphics_pixels + 3
.Llow320:
	shl	ah, 1
	shl	ah, 1
	test	byte ptr [bx], 1
	jz	1f
	or	ah, 3
1:
	dec	bx
	cmp	bx, graphics_pixels - 1
	jne	.Llow320
	mov	al, ah
	call	send
	mov	al, byte ptr [graphics_first]	; the first byte again
	call	send
	inc	cx
	cmp	cx, word ptr [graphics_count]
	jne	.Lcolumn320
.Lnext320:
	call	new_line
	add	dx, 4
	cmp	dl, 200
	jne	.Lband320
	jmp	.Ldone

; 640 x 200: CX = the band's right column
.L640:
	mov	cx, 639
.Lband640:
	push	cx
	mov	dx, 199
.Lscan640:
	pop	cx
	push	cx
	mov	bl, 8
.Lscan640_column:
	mov	ah, 0x0d
	int	0x10
	cmp	al, 0
	jne	.Lfound640
	dec	cx
	dec	bl
	jnz	.Lscan640_column
	cmp	dl, 0
	je	.Lempty640			; no pixel in the band: only the new line
	dec	dx
	jmp	.Lscan640
.Lempty640:
	pop	cx
	jmp	.Lnext640
.Lfound640:
	inc	dx
	mov	word ptr [graphics_count], dx
	call	bit_image			; leaves DX = 0: the first row
	pop	cx
.Lrow640:
	push	cx
	mov	al, 0
	mov	bl, 8
.Lbit640:
	push	ax
	mov	ah, 0x0d
	int	0x10
	cmp	al, 0				; CF = 0
	je	1f
	cmc					; a dot: CF = 1
1:
	pop	ax
	rcl	al, 1
	dec	cx
	dec	bl
	jnz	.Lbit640
	; ASSUMPTION (spec A1): INT 17h keeps AL, so the byte goes out four times as the original's
	call	send				; four dot columns
	call	send
	call	send
	call	send
	pop	cx
	inc	dx
	cmp	dx, word ptr [graphics_count]
	jne	.Lrow640
.Lnext640:
	call	new_line
	sub	cx, 8
	cmp	cx, 0
	jge	.Lband640

.Ldone:
	xor	dx, dx
	mov	al, 0x1b			; ESC 3 36: 1/6-inch lines again
	call	send
	mov	al, '3'
	call	send
	mov	al, 36
	call	send
.Lstatus:					; DH = the status byte
	mov	ax, 0x50
	mov	ds, ax
	mov	byte ptr [0], dh
.Lreturn:
	pop	bx
	pop	cx
	pop	dx
	pop	ds
	pop	ax
	iret

; ZF = 0 if one of the pixels (CX, DX) to (CX, DX + 3) is not 0 (read from the top, up to the
; first that is not 0).
any_of_four:
	push	dx
	mov	bl, 4
1:
	mov	ah, 0x0d
	int	0x10
	cmp	al, 0
	jne	2f
	inc	dx
	dec	bl
	jnz	1b
2:
	pop	dx
	ret

; ESC 3 24: lines of 24/216 inch, the eight pins.
spacing_24:
	mov	al, 0x1b
	call	send
	mov	al, '3'
	call	send
	mov	al, 24
	jmp	send

; ESC L n1 n2 with n = the band's length x 3 (320 x 200) or x 4 (640 x 200); DX = 0 after it.
bit_image:
	xor	dx, dx
	mov	al, 0x1b
	call	send
	mov	al, 'L'
	call	send
	push	cx
	mov	ax, word ptr [graphics_count]
	mov	cx, 3
	cmp	byte ptr [graphics_mode320], 0
	jne	1f
	inc	cx
1:
	mul	cx
	xor	dx, dx
	mov	cx, ax
	mov	al, cl
	call	send
	mov	al, ch
	call	send
	pop	cx
	ret

; LF, CR.
new_line:
	mov	al, 0x0a
	call	send
	mov	al, 0x0d
	; fall into send

; INT 17h function 0 on printer 0 with AL, which it keeps; a status with 25h (time-out, out
; of paper) ends the print: SP as at the handler's start, status FFh. Keeps DX.
send:
	push	dx
	xor	dx, dx
	mov	ah, 0
	int	0x17
	pop	dx
	test	ah, 0x25
	jnz	1f
	ret
1:
	mov	sp, word ptr [graphics_sp]
	mov	dh, 0xff
	jmp	.Lstatus

; The transient part. DOS jumped to 100h with CS = DS = ES = SS = the PSP's segment. As
; crt0_com.s: SP at the end of the program's memory (at most 0FFF0h), .bss cleared, main called,
; INT 20h (main does not come back: it ends with INT 27h).
	.org	0x235
	.globl	graphics_start
graphics_start:					; 335h
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

; void graphics_keep(u16 bytes) -> !: INT 27h with DX = bytes (CS is the PSP's segment).
	.section	.text.graphics_keep,"ax",@progbits
	.globl	graphics_keep
graphics_keep:
	mov	dx, ax
	int	0x27
