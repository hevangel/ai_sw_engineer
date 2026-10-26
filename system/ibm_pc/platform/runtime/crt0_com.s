; Start of a .COM program under PC DOS (linked with ../link_com.ld).
;
; DOS jumps to CS:0100h with CS = DS = ES = SS = the segment of the program
; segment prefix and SP at the top of the segment. The word at PSP:0002h is
; the first paragraph past the memory the program may use. This code
;   1. puts SP at the end of that memory (at most 0FFF0h: one segment);
;   2. clears .bss;
;   3. calls main and ends the program with INT 20h.

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	cld
	mov	ax, word ptr [2]	; first paragraph past the program's memory
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
