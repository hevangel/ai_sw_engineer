; PRNCAP.COM: captures what is sent to printer 0 (graphics/tools/README.md). This repository's own
; program, not IBM's: it stays resident (INT 21h function 31h) with a 96 KiB buffer and hooks
; INT 17h:
;
;   DX = 0, AH = 0      AL is stored at the next free byte of the buffer (counted in `lost`
;                       once the buffer is full); AH = the status, AL kept
;   DX = 0, AH = 1, 2   AH = the status
;   AH = 0E3h, BX = 5043h ('CP')   the private function, any DX: BX = 4350h ('PC'),
;                       ES = the buffer's segment, DX:AX = the bytes stored, SI:CX = the
;                       bytes lost; AL = 1 on entry empties the buffer first
;   anything else       the INT 17h handler found at the start
;
; The status is 90h (not busy, selected); `prncap e` makes it 30h (busy, selected, out of
; paper), which a caller testing AH with 25h sees as an error. A second run finds the
; resident copy through the private function and only says so.

	.set	MAGIC, 0x5043
	.set	ANSWER, 0x4350
	.set	BUF_PARAGRAPHS, 0x1800		; 96 KiB
	.set	BUF_HI, 1			; 96 KiB = 1:8000h bytes
	.set	BUF_LO, 0x8000

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	jmp	install

old17:	.word	0, 0
count:	.word	0, 0				; low, high
lost:	.word	0, 0
bufseg:	.word	0
status:	.byte	0x90

int17:
	cmp	ah, 0xe3
	jne	.Lprinter
	cmp	bx, MAGIC
	jne	.Lprinter
	mov	bx, ANSWER
	cmp	al, 1
	jne	.Lquery
	mov	word ptr cs:[count], 0
	mov	word ptr cs:[count + 2], 0
	mov	word ptr cs:[lost], 0
	mov	word ptr cs:[lost + 2], 0
.Lquery:
	mov	ax, word ptr cs:[count]
	mov	dx, word ptr cs:[count + 2]
	mov	cx, word ptr cs:[lost]
	mov	si, word ptr cs:[lost + 2]
	mov	es, word ptr cs:[bufseg]
	iret
.Lprinter:
	cmp	dx, 0
	jne	.Lchain
	cmp	ah, 0
	je	.Lstore
	cmp	ah, 2
	ja	.Lchain
	mov	ah, byte ptr cs:[status]
	iret
.Lchain:
	.byte	0x2e, 0xff, 0x2e		; JMP FAR CS:[old17]
	.word	old17

.Lstore:
	push	bx
	push	cx
	push	di
	push	es
	mov	bx, word ptr cs:[count]
	mov	cx, word ptr cs:[count + 2]
	cmp	cx, BUF_HI
	jb	.Lroom
	ja	.Lfull
	cmp	bx, BUF_LO
	jb	.Lroom
.Lfull:
	add	word ptr cs:[lost], 1
	adc	word ptr cs:[lost + 2], 0
	jmp	.Lstored
.Lroom:
	push	ax				; the byte's segment: bufseg + count / 16
	mov	di, cx
	mov	cl, 12
	shl	di, cl
	mov	ax, bx
	mov	cl, 4
	shr	ax, cl
	add	ax, di
	add	ax, word ptr cs:[bufseg]
	mov	es, ax
	mov	di, bx
	and	di, 15
	pop	ax
	mov	byte ptr es:[di], al
	add	word ptr cs:[count], 1
	adc	word ptr cs:[count + 2], 0
.Lstored:
	mov	ah, byte ptr cs:[status]
	pop	es
	pop	di
	pop	cx
	pop	bx
	iret

	.p2align	4
resident_end:

; The start: CS = DS = ES = SS = the PSP's segment.
install:
	mov	ax, 0xe300
	mov	bx, MAGIC
	xor	dx, dx
	int	0x17
	cmp	bx, ANSWER
	jne	.Lnew
	mov	dx, msg_already
	mov	ah, 9
	int	0x21
	int	0x20
.Lnew:
	mov	si, 0x81			; `e` anywhere in the tail: the error status
.Ltail:
	lodsb
	cmp	al, 13
	je	.Ltail_done
	or	al, 0x20
	cmp	al, 'e'
	jne	.Ltail
	mov	byte ptr [status], 0x30
.Ltail_done:
	mov	ax, resident_end		; the buffer: the paragraphs after resident_end
	mov	cl, 4
	shr	ax, cl
	mov	di, ax				; DI = the resident paragraphs
	mov	bx, cs
	add	ax, bx
	mov	word ptr [bufseg], ax
	add	ax, BUF_PARAGRAPHS
	cmp	ax, word ptr [2]		; the top of memory (PSP)
	jbe	.Lfits
	mov	dx, msg_memory
	mov	ah, 9
	int	0x21
	int	0x20
.Lfits:
	mov	ax, 0x3517
	int	0x21
	mov	word ptr [old17], bx
	mov	word ptr [old17 + 2], es
	mov	dx, int17
	mov	ax, 0x2517
	int	0x21
	mov	dx, msg_installed
	mov	ah, 9
	int	0x21
	mov	dx, di
	add	dx, BUF_PARAGRAPHS
	mov	ax, 0x3100
	int	0x21

msg_installed:
	.ascii	"PRNCAP installed\r\n$"
msg_already:
	.ascii	"PRNCAP already installed\r\n$"
msg_memory:
	.ascii	"PRNCAP: not enough memory\r\n$"
