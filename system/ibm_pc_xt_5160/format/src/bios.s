; The calls FORMAT makes that the shared ibm_pc_dos crate has no function for (spec D3):
; INT 25h/26h, INT 11h and INT 13h. Arguments and results follow llvm-i8086/spec/abi.md D3;
; BX, SI, DI and BP are preserved, and so are DS and ES.

; u16 format_disk_io(struct Io *io)
;   struct Io { u16 write, drive, buf, count, sector; }  (offsets 0, 2, 4, 6, 8)
; INT 25h (write == 0) or INT 26h: AL = drive (0 = A), DS:BX = buf, CX = count, DX = the first
; logical sector. Both return with the caller's flags still on the stack (removed here) and
; may change every register but the segment registers. Returns 0, or 100h + AL (the error
; code) if the carry was set.
	.section	.text.format_disk_io,"ax",@progbits
	.globl	format_disk_io
format_disk_io:
	push	bx
	push	si
	push	di
	push	bp
	push	ds
	push	es
	mov	si, ax
	mov	bx, word ptr [si + 4]
	mov	cx, word ptr [si + 6]
	mov	dx, word ptr [si + 8]
	mov	al, byte ptr [si + 2]
	cmp	word ptr [si], 0
	jne	.Lwrite
	int	0x25
	jmp	.Ldone
.Lwrite:
	int	0x26
.Ldone:
	pop	si			; the flags INT 25h/26h leave on the stack (POP keeps CF)
	jc	.Lerror
	xor	ax, ax
	jmp	.Lout
.Lerror:
	mov	ah, 1
.Lout:
	pop	es
	pop	ds
	pop	bp
	pop	di
	pop	si
	pop	bx
	ret

; u16 format_equipment(void): INT 11h, the BIOS equipment word.
	.section	.text.format_equipment,"ax",@progbits
	.globl	format_equipment
format_equipment:
	push	bx
	push	bp
	int	0x11
	pop	bp
	pop	bx
	ret

; void format_bios_disk(struct Bios *b)
;   struct Bios { u16 ax, cx, dx, table, carry; }  (offsets 0, 2, 4, 6, 8)
; INT 13h with AX, CX, DX; AX, DX and the carry (0/1) are stored back. With `table` not 0 the
; 36 bytes there are exchanged with 0060:0000 before the call, which gets ES:BX = 0060:0000,
; and exchanged back after it, as the original's 12CBh does for AH = 05h.
	.section	.text.format_bios_disk,"ax",@progbits
	.globl	format_bios_disk
format_bios_disk:
	push	bx
	push	si
	push	di
	push	bp
	push	ds
	push	es
	mov	bp, ax			; the parameter block
	mov	si, word ptr [bp + 6]
	or	si, si
	jz	.Lcall
	call	.Lswap
.Lcall:
	mov	ax, 0x60
	mov	es, ax
	xor	bx, bx
	mov	ax, word ptr [bp + 0]
	mov	cx, word ptr [bp + 2]
	mov	dx, word ptr [bp + 4]
	push	bp
	int	0x13
	pop	bp
	push	ds
	pop	es
	mov	word ptr [bp + 0], ax
	mov	word ptr [bp + 4], dx
	mov	ax, 0
	adc	ax, 0
	mov	word ptr [bp + 8], ax
	mov	si, word ptr [bp + 6]
	or	si, si
	jz	.Lbios_out
	call	.Lswap
.Lbios_out:
	pop	es
	pop	ds
	pop	bp
	pop	di
	pop	si
	pop	bx
	ret
; exchanges the 18 words at DS:SI with 0060:0000 (keeps BP)
.Lswap:
	push	es
	mov	ax, 0x60
	mov	es, ax
	xor	di, di
	mov	cx, 18
.Lswap_word:
	lodsw
	mov	bx, word ptr es:[di]
	stosw
	mov	word ptr [si - 2], bx
	loop	.Lswap_word
	pop	es
	ret
