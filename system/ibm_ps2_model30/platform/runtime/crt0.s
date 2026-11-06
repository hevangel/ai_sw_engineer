; Boot sector and C runtime start for the IBM PS/2 Model 30 profile, linked with
; ../link.ld. The same contract as system/ibm_pc/platform/runtime/crt0.s (the
; tiny model, the stack below 7C00h, .bss cleared, main called), for the Model
; 30's 3.5-inch 720 KiB diskette: 80 cylinders, 2 heads, 9 sectors per track.
;
; The BIOS loads this sector at 0000:7C00 with DL = the boot drive. It
;   1. sets CS = DS = ES = SS = 0 and the stack below the boot sector;
;   2. reads the rest of the image, from logical sector 1 on, to 0000:7E00 one
;      sector at a time with INT 13h, in the disk's logical order (sector 1..9,
;      then head 1, then the next cylinder), retrying three times; the drive is
;      the one the BIOS booted from (DL);
;   3. clears .bss;
;   4. calls main. If main returns the machine halts.
;
; i8086-ld --floppy-kib 720 writes the image in logical sector order. The section holds code
; only (no data bytes), so the independent decoders can check every byte of it.

SPT = 9					; sectors per track
HEADS = 2

	.section	.boot,"ax",@progbits
	.globl	_start
_start:
	cli
	xor	ax, ax
	mov	ds, ax
	mov	es, ax
	mov	ss, ax
	mov	sp, _stack_top
	sti
	cld
	mov	bp, dx			; BP = the boot drive (DL); INT 13h keeps BP
	mov	ax, __data_end		; sectors = ceil((__data_end - 0x7E00) / 512)
	sub	ax, 0x7e00
	add	ax, 511
	mov	cl, 9
	shr	ax, cl
	mov	si, ax			; sectors left to read
	mov	bx, 0x7e00		; ES:BX = where the next one goes
	mov	cx, 0x0002		; cylinder 0, sector 2
	xor	dh, dh			; head 0
.Lread:
	test	si, si
	jz	.Lloaded
	mov	di, 3			; attempts
.Ltry:
	mov	ax, bp
	mov	dl, al			; the boot drive
	mov	ax, 0x0201		; INT 13h: read one sector
	push	dx
	int	0x13
	pop	dx
	jnc	.Lok
	xor	ax, ax			; reset the controller and try again (DL is still the drive)
	push	dx
	int	0x13
	pop	dx
	dec	di
	jnz	.Ltry
	jmp	.Lhalt
.Lok:
	add	bx, 512
	inc	cl
	cmp	cl, SPT + 1
	jb	.Lnext
	mov	cl, 1			; next head, or next cylinder
	inc	dh
	cmp	dh, HEADS
	jb	.Lnext
	xor	dh, dh
	inc	ch
.Lnext:
	dec	si
	jmp	.Lread
.Lloaded:
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	call	main
.Lhalt:
	cli
.Lspin:
	hlt
	jmp	.Lspin
