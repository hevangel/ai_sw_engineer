; DRAW.COM <mode>[b]: draws the test picture of graphics/tools/README.md in BIOS video mode
; <mode> (0-6), calls INT 5 (Print Screen), then sets mode 3 and shows the print screen status
; byte (0050:0000h). This repository's own program, not IBM's. `b` after the mode sets the
; status byte to 1 (a print screen in progress) before INT 5.
;
; Modes 4, 5 (320 x 200, W = 320, S = 0) and 6 (640 x 200, W = 640, S = 1, colour c drawn as
; c != 0), written straight into the CGA memory at B800h; x << S is written x*:
;   E1 corners   (0,0) 1, (W-1,0) 2, (0,199) 3, (W-1,199) 1
;   E2 bars      colour c = 0-3: x (8+20c)* to (24+20c)* (excluded), y 8-23
;   E3 line      y 30, x 1 to W/2 (excluded), colour 3
;   E4 diagonal  y 40-159: x = (2(y-40))*, colour 1 + y mod 3
;   E5 diagonal  y 40-159: x = W/2 + (y-40), colour 2
;   E6 box       outline x 200* to 300* (included), y 100 to 150, colour 2
;   E7 block     x 220* to 240*, y 120 to 130 (excluded), colour 3
;   E8 probe     (250*, 163) colour 1
;   E9 text      `GRAPHICS` written by the BIOS (INT 10h 0Eh, BL = 3) from text row 21, column 1
; Modes 0-3 (40 or 80 columns, C): the text memory: row 0 `DRAW text mode <m>` (the mode a
; digit), from row 2 the characters 0-255 in order, C to a row, row 24 `last row` ending at
; column C-1, every attribute 07h.

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	cld
	mov	si, 0x81
.Lskip:
	lodsb
	cmp	al, ' '
	je	.Lskip
	sub	al, '0'
	cmp	al, 6
	jbe	.Lmode
	mov	dx, msg_usage
	mov	ah, 9
	int	0x21
	int	0x20
.Lmode:
	mov	byte ptr [mode], al
	lodsb
	or	al, 0x20
	cmp	al, 'b'
	jne	.Lset
	mov	byte ptr [busy], 1
.Lset:
	mov	al, byte ptr [mode]
	mov	ah, 0
	int	0x10
	mov	ax, 0xb800
	mov	es, ax
	cmp	byte ptr [mode], 4
	jb	.Ltext
	call	picture
	jmp	.Lprint
.Ltext:
	call	text
.Lprint:
	xor	ax, ax
	mov	es, ax
	cmp	byte ptr [busy], 0
	je	.Lint5
	mov	byte ptr es:[0x500], 1
.Lint5:
	int	0x05
	xor	ax, ax
	mov	es, ax
	mov	al, byte ptr es:[0x500]
	mov	byte ptr [status], al
	mov	ax, 3
	int	0x10
	mov	dx, msg_status
	mov	ah, 9
	int	0x21
	mov	al, byte ptr [status]
	push	ax
	mov	cl, 4
	shr	al, cl
	call	hexdigit
	pop	ax
	call	hexdigit
	mov	dx, msg_end
	mov	ah, 9
	int	0x21
	int	0x20

; Shows the low nibble of AL as a hex digit.
hexdigit:
	and	al, 15
	add	al, '0'
	cmp	al, '9'
	jbe	1f
	add	al, 7
1:
	mov	dl, al
	mov	ah, 2
	int	0x21
	ret

; The text picture (modes 0-3), ES = B800h.
text:
	mov	ax, 40
	cmp	byte ptr [mode], 2
	jb	1f
	mov	ax, 80
1:
	mov	word ptr [columns], ax
	xor	di, di				; row 0
	mov	si, msg_textmode
	call	put
	mov	al, byte ptr [mode]
	add	al, '0'
	mov	ah, 7
	stosw
	mov	ax, word ptr [columns]		; row 2: the characters 0-255
	shl	ax, 1
	shl	ax, 1
	mov	di, ax
	xor	ax, ax
	mov	ah, 7
.Lchars:
	stosw
	inc	al
	jnz	.Lchars
	mov	ax, word ptr [columns]		; row 24, ending at column C-1
	mov	dx, 25
	mul	dx
	sub	ax, 8
	shl	ax, 1
	mov	di, ax
	mov	si, msg_lastrow
	call	put
	ret

; Stores the zero-terminated text at SI with attribute 07h at ES:DI.
put:
	lodsb
	or	al, al
	jz	1f
	mov	ah, 7
	stosw
	jmp	put
1:
	ret

; The graphics picture (modes 4-6), ES = B800h.
picture:
	mov	byte ptr [shift], 0
	mov	word ptr [width], 320
	cmp	byte ptr [mode], 6
	jne	1f
	mov	byte ptr [shift], 1
	mov	word ptr [width], 640
1:
	; E1 corners
	xor	cx, cx
	xor	dx, dx
	mov	al, 1
	call	plot
	mov	cx, word ptr [width]
	dec	cx
	mov	al, 2
	call	plot
	mov	dx, 199
	mov	al, 1
	call	plot
	xor	cx, cx
	mov	al, 3
	call	plot
	; E2 bars
	xor	bl, bl				; the colour
.Lbar:
	mov	al, 20
	mul	bl
	add	ax, 8
	mov	cx, ax
	call	scale
	mov	word ptr [x0], cx
	add	ax, 16
	mov	cx, ax
	call	scale
	mov	word ptr [x1], cx
	mov	word ptr [y0], 8
	mov	word ptr [y1], 24
	mov	al, bl
	call	fill
	inc	bl
	cmp	bl, 4
	jb	.Lbar
	; E3 line
	mov	word ptr [x0], 1
	mov	ax, word ptr [width]
	shr	ax, 1
	mov	word ptr [x1], ax
	mov	word ptr [y0], 30
	mov	word ptr [y1], 31
	mov	al, 3
	call	fill
	; E4 and E5 diagonals
	mov	dx, 40
.Ldiagonal:
	mov	cx, dx
	sub	cx, 40
	shl	cx, 1
	call	scale
	mov	ax, dx				; colour 1 + y mod 3
	mov	bl, 3
	div	bl
	mov	al, ah
	inc	al
	call	plot
	mov	cx, word ptr [width]
	shr	cx, 1
	add	cx, dx
	sub	cx, 40
	mov	al, 2
	call	plot
	inc	dx
	cmp	dx, 160
	jb	.Ldiagonal
	; E6 box
	mov	cx, 200
	call	scale
	mov	word ptr [x0], cx
	mov	cx, 301
	call	scale
	sub	cx, word ptr [shift]		; to 300* included
	mov	word ptr [x1], cx
	mov	word ptr [y0], 100
	mov	word ptr [y1], 101
	mov	al, 2
	call	fill
	mov	word ptr [y0], 150
	mov	word ptr [y1], 151
	mov	al, 2
	call	fill
	mov	ax, word ptr [x1]
	push	ax
	mov	ax, word ptr [x0]
	inc	ax
	mov	word ptr [x1], ax
	mov	word ptr [y0], 100
	mov	word ptr [y1], 151
	mov	al, 2
	call	fill
	pop	ax
	mov	word ptr [x1], ax
	dec	ax
	mov	word ptr [x0], ax
	mov	al, 2
	call	fill
	; E7 block
	mov	cx, 220
	call	scale
	mov	word ptr [x0], cx
	mov	cx, 240
	call	scale
	mov	word ptr [x1], cx
	mov	word ptr [y0], 120
	mov	word ptr [y1], 130
	mov	al, 3
	call	fill
	; E8 probe
	mov	cx, 250
	call	scale
	mov	dx, 163
	mov	al, 1
	call	plot
	; E9 text
	mov	ah, 2
	xor	bh, bh
	mov	dx, 0x1501			; row 21, column 1
	int	0x10
	mov	si, msg_graphics
.Llabel:
	lodsb
	or	al, al
	jz	.Ltext_done
	mov	ah, 0x0e
	mov	bx, 3
	int	0x10
	jmp	.Llabel
.Ltext_done:
	ret

; CX <<= [shift].
scale:
	push	ax
	mov	al, byte ptr [shift]
	or	al, al
	jz	1f
	shl	cx, 1
1:
	pop	ax
	ret

; Fills x0 <= x < x1, y0 <= y < y1 with colour AL.
fill:
	push	cx
	push	dx
	mov	dx, word ptr [y0]
.Lrow:
	cmp	dx, word ptr [y1]
	jae	.Lfilled
	mov	cx, word ptr [x0]
.Lcolumn:
	cmp	cx, word ptr [x1]
	jae	.Lnext_row
	call	plot
	inc	cx
	jmp	.Lcolumn
.Lnext_row:
	inc	dx
	jmp	.Lrow
.Lfilled:
	pop	dx
	pop	cx
	ret

; Sets the pixel (CX, DX) to colour AL in the CGA memory at ES (two bits a pixel at 320 x 200,
; one bit, AL != 0, at 640 x 200; even rows from 0000h, odd rows from 2000h, 80 bytes a row).
plot:
	push	ax
	push	bx
	push	cx
	push	dx
	push	di
	mov	bl, al
	mov	ax, dx
	shr	ax, 1
	mov	di, 80
	push	dx
	mul	di
	pop	dx
	mov	di, ax
	test	dl, 1
	jz	1f
	add	di, 0x2000
1:
	cmp	byte ptr [shift], 0
	jne	.Lplot640
	mov	ax, cx
	shr	ax, 1
	shr	ax, 1
	add	di, ax
	and	cl, 3
	mov	al, 3
	sub	al, cl
	shl	al, 1
	mov	cl, al
	and	bl, 3
	shl	bl, cl
	mov	al, 3
	shl	al, cl
	not	al
	and	byte ptr es:[di], al
	or	byte ptr es:[di], bl
	jmp	.Lplotted
.Lplot640:
	mov	ax, cx
	shr	ax, 1
	shr	ax, 1
	shr	ax, 1
	add	di, ax
	and	cl, 7
	mov	al, 7
	sub	al, cl
	mov	cl, al
	mov	al, 1
	shl	al, cl
	or	bl, bl
	jz	.Lclear
	or	byte ptr es:[di], al
	jmp	.Lplotted
.Lclear:
	not	al
	and	byte ptr es:[di], al
.Lplotted:
	pop	di
	pop	dx
	pop	cx
	pop	bx
	pop	ax
	ret

msg_usage:
	.ascii	"usage: DRAW mode (0-6), then b for busy\r\n$"
msg_status:
	.ascii	"INT 5 done, status $"
msg_end:
	.ascii	"h\r\n$"
msg_textmode:
	.asciz	"DRAW text mode "
msg_lastrow:
	.asciz	"last row"
msg_graphics:
	.asciz	"GRAPHICS"

mode:	.byte	0
busy:	.byte	0
status:	.byte	0
shift:	.byte	0, 0
	.p2align	1
width:	.word	0
columns: .word	0
x0:	.word	0
x1:	.word	0
y0:	.word	0
y1:	.word	0
