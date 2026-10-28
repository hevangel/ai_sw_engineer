; A two-stage boot program for the IBM PC 5150, linked with
; system/ibm_pc/platform/link.ld by i8086-ld into a bootable floppy image.
;
; Stage 1 is the boot sector (.boot, loaded at 0000:7C00 by the BIOS): it sets
; up the segments and the stack, reads the next sector from drive A with
; INT 13h into 0000:7E00, where the linker put .text, and jumps to stage 2.
; Stage 2 bumps a counter in .bss three times, copies a string from .rodata to
; the CGA text screen (B800:0000) and halts.
;
; tests/boot.rs-style check in system/ibm_pc/emulator/tests/i8086_codegen.rs:
; the image layout is asserted always, and the whole boot through the original
; BIOS when the ROMs are available.

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
	mov	ah, 2			; INT 13h: read sectors
	mov	al, 1			; one sector
	mov	ch, 0			; track 0
	mov	cl, 2			; sector 2 (the one after this)
	mov	dh, 0			; head 0
	mov	dl, 0			; drive A
	mov	bx, 0x7e00		; ES:BX = 0000:7E00
	int	0x13
	jc	.Lfail
	jmp	stage2
.Lfail:
	cli
.Lspin:
	hlt
	jmp	.Lspin

	.section	.text.stage2,"ax",@progbits
	.globl	stage2
stage2:
	call	bump
	call	bump
	call	bump
	mov	ax, 0xb800
	mov	es, ax
	xor	di, di
	mov	si, message
	mov	ah, 0x07		; light grey on black
.Lnext:
	lodsb
	test	al, al
	jz	.Ldone
	stosw
	jmp	.Lnext
.Ldone:
	cli
.Lhalt:
	hlt
	jmp	.Lhalt

	.section	.text.bump,"ax",@progbits
	.globl	bump
bump:
	inc	word ptr [counter]
	ret

; Never called: the linker's section collection must drop it.
	.section	.text.unused,"ax",@progbits
	.globl	unused
unused:
	mov	ax, 0xdead
	ret

	.section	.rodata.message,"a",@progbits
message:
	.asciz	"llvm-i8086: stage 2 ok"

	.section	.bss.counter,"aw",@nobits
	.globl	counter
	.p2align	1
counter:
	.zero	2
