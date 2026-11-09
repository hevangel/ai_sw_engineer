; PRNDUMP.COM <file>: writes the bytes PRNCAP.COM captured to <file> and empties the capture
; (graphics/tools/README.md). This repository's own program, not IBM's. It finds the buffer
; with PRNCAP's private INT 17h function (AH = 0E3h, BX = 5043h), creates the file (function
; 3Ch), writes the bytes in pieces of at most 32 KiB (function 40h), closes it, empties the
; buffer (AL = 1) and shows `<n> bytes`, then ` (<m> lost)` if the buffer overflowed.

	.set	MAGIC, 0x5043
	.set	ANSWER, 0x4350
	.set	PIECE, 0x8000

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	cld
	mov	si, 0x81			; the file name: the tail's first word
.Lskip:
	lodsb
	cmp	al, ' '
	je	.Lskip
	cmp	al, 9
	je	.Lskip
	cmp	al, 13
	jne	.Lname
	mov	dx, msg_usage
	jmp	fail
.Lname:
	mov	di, name
.Lcopy:
	stosb
	lodsb
	cmp	al, ' '
	ja	.Lcopy
	mov	byte ptr [di], 0

	mov	ax, 0xe300
	mov	bx, MAGIC
	xor	dx, dx
	int	0x17
	cmp	bx, ANSWER
	je	.Lfound
	mov	dx, msg_none
	jmp	fail
.Lfound:
	mov	word ptr [count], ax
	mov	word ptr [count + 2], dx
	mov	word ptr [lost], cx
	mov	word ptr [lost + 2], si
	mov	word ptr [segment], es
	push	ds
	pop	es

	mov	ah, 0x3c
	xor	cx, cx
	mov	dx, name
	int	0x21
	jnc	.Lcreated
	mov	dx, msg_create
	jmp	fail
.Lcreated:
	mov	bx, ax				; the handle, kept in BX
	mov	ax, word ptr [count]
	mov	dx, word ptr [count + 2]
.Lpiece:
	mov	cx, PIECE			; CX = min(DX:AX, PIECE)
	or	dx, dx
	jnz	.Lwrite
	cmp	ax, cx
	jae	.Lwrite
	mov	cx, ax
	jcxz	.Lwritten
.Lwrite:
	push	ax
	push	dx
	push	cx
	push	ds
	mov	ds, word ptr [segment]
	xor	dx, dx
	mov	ah, 0x40
	int	0x21
	pop	ds
	pop	cx
	jc	.Lwrite_error
	cmp	ax, cx
	jne	.Lwrite_error
	pop	dx
	pop	ax
	sub	ax, cx
	sbb	dx, 0
	add	word ptr [segment], PIECE / 16
	jmp	.Lpiece
.Lwrite_error:
	mov	dx, msg_write
	jmp	fail
.Lwritten:
	mov	ah, 0x3e
	int	0x21
	mov	ax, 0xe301			; empty the buffer
	mov	bx, MAGIC
	xor	dx, dx
	int	0x17

	mov	ax, word ptr [count]
	mov	dx, word ptr [count + 2]
	call	decimal
	mov	dx, msg_bytes
	mov	ah, 9
	int	0x21
	mov	ax, word ptr [lost]
	mov	dx, word ptr [lost + 2]
	mov	cx, ax
	or	cx, dx
	jz	.Lend
	push	dx
	push	ax
	mov	dl, ' '
	mov	ah, 2
	int	0x21
	mov	dl, '('
	mov	ah, 2
	int	0x21
	pop	ax
	pop	dx
	call	decimal
	mov	dx, msg_lost
	mov	ah, 9
	int	0x21
.Lend:
	mov	dx, msg_crlf
	mov	ah, 9
	int	0x21
	int	0x20

fail:
	mov	ah, 9
	int	0x21
	int	0x20

; Shows DX:AX in decimal (function 02h).
decimal:
	mov	cx, 10
	xor	bx, bx				; the digits pushed
.Ldivide:
	push	ax				; DX:AX / 10, the remainder pushed
	mov	ax, dx
	xor	dx, dx
	div	cx
	mov	si, ax
	pop	ax
	div	cx
	push	dx
	inc	bx
	mov	dx, si
	mov	di, ax
	or	di, dx
	jnz	.Ldivide
.Lshow:
	pop	dx
	add	dl, '0'
	mov	ah, 2
	int	0x21
	dec	bx
	jnz	.Lshow
	ret

msg_bytes:
	.ascii	" bytes$"
msg_lost:
	.ascii	" lost)$"
msg_crlf:
	.ascii	"\r\n$"
msg_usage:
	.ascii	"usage: PRNDUMP file\r\n$"
msg_none:
	.ascii	"PRNCAP is not installed\r\n$"
msg_create:
	.ascii	"PRNDUMP: cannot create the file\r\n$"
msg_write:
	.ascii	"PRNDUMP: write error\r\n$"

count:	.word	0, 0
lost:	.word	0, 0
segment: .word	0
name:	.space	80
