; The absolute disk calls CHKDSK makes that the shared ibm_pc_dos crate has no function for
; (spec D3). Arguments and results follow llvm-i8086/spec/abi.md D3; BX, SI, DI and BP are
; preserved, and so are DS and ES.

; u16 chkdsk_disk_io(struct Io *io)
;   struct Io { u16 write, drive, buf, count, sector; }  (offsets 0, 2, 4, 6, 8)
; INT 25h (write == 0) or INT 26h: AL = drive (0 = A), DS:BX = buf, CX = count, DX = the first
; logical sector. Both return with the caller's flags still on the stack (removed here) and
; may change every register but the segment registers. Returns 0, or 100h + AL (the error
; code) if the carry was set.
	.section	.text.chkdsk_disk_io,"ax",@progbits
	.globl	chkdsk_disk_io
chkdsk_disk_io:
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
