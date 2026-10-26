; Boot sector and C runtime start for programs linked with ../link.ld.
;
; The BIOS loads this sector (the .boot section, KEEP in the script) at
; 0000:7C00 with DL = the boot drive and jumps to it. It
;   1. sets CS = DS = ES = SS = 0 (the tiny model) and the stack below the
;      boot sector (_stack_top = 0x7C00);
;   2. reads the rest of the image, from sector 2 on, to 0000:7E00 one sector at a time with
;      INT 13h (a 5150 floppy has 8 sectors per track, one side), retrying three times;
;   3. clears .bss;
;   4. calls main. If main returns the machine halts.
;
; The number of sectors comes from the linker: __data_end is the end of the
; last loaded section. Drive A (0) is assumed, as the program image is the
; floppy the BIOS booted from.

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
	mov	ax, __data_end		; sectors = ceil((__data_end - 0x7E00) / 512)
	sub	ax, 0x7e00
	add	ax, 511
	mov	cl, 9
	shr	ax, cl
	mov	si, ax			; sectors left to read
	mov	bx, 0x7e00		; ES:BX = where the next one goes
	mov	cx, 0x0002		; track 0, sector 2
.Lread:
	test	si, si
	jz	.Lloaded
	mov	di, 3			; attempts
.Ltry:
	mov	ax, 0x0201		; INT 13h: read one sector
	xor	dx, dx			; head 0, drive A
	int	0x13
	jnc	.Lok
	xor	ax, ax			; reset the controller and try again
	xor	dx, dx
	int	0x13
	dec	di
	jnz	.Ltry
	jmp	.Lhalt
.Lok:
	add	bx, 512
	inc	cl
	cmp	cl, 9
	jb	.Lnext
	mov	cl, 1			; next track
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
