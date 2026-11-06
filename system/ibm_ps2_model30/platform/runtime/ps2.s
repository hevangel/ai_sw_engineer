; PS/2 Model 30 additions to the shared HAL (system/ibm_pc/platform/runtime/hal.s, which this
; profile links unchanged). Arguments and preserved registers per llvm-i8086/spec/abi.md D3.

; u16 ps2_display_combination(void): INT 10h AX=1A00h ("read display combination code",
; IBM PS/2 BIOS). Returns BL (the active display) in the low byte and BH (the alternate) in the
; high byte when the BIOS supports the function (it returns AL = 1Ah), else 0FFFFh.
; Codes: 0Bh MCGA with a monochrome analog display, 0Ch MCGA with a colour analog display.
	.section	.text.ps2_display_combination,"ax",@progbits
	.globl	ps2_display_combination
ps2_display_combination:
	push	bx
	push	si
	push	di
	push	bp
	mov	ax, 0x1a00
	int	0x10
	cmp	al, 0x1a
	mov	ax, bx
	je	.Ldone
	mov	ax, 0xffff
.Ldone:
	pop	bp
	pop	di
	pop	si
	pop	bx
	ret
