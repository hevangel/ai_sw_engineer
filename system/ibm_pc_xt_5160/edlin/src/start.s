; EDLIN's start, in place of the shared runtime/crt0_com.s (spec D1).
;
; The rewrite is a load-high .EXE named EDLIN.COM (scripts/mkexe.py: MINALLOC = MAXALLOC = 0),
; so DOS gives it all memory, as it gives a .COM, but places the program at the top (just below
; the EXEC code of COMMAND's transient part) and leaves the program segment prefix's 64 KiB, the original's
; whole world, untouched: the core keeps the original's data and text there at the original's
; offsets. DOS starts the program with DS = ES = the PSP's segment and CS = SS = the image's
; paragraph - 10h (the header's CS and SS are FFF0h), so the image is linked at 100h as a
; .COM (link_com.ld). This code
;   0. keeps AX (dos_entry_ax) and the PSP's segment (edlin_seg);
;   1. sets DS = ES = SS = CS and SP to the end of the stack after .bss (STACK bytes; the
;      file holds .bss and the stack as zeros, since memory past the image is not the
;      program's when it is loaded high);
;   2. clears .bss, calls main and ends the program (edlin_exit: INT 20h with CS = the PSP's
;      segment, as the original's INT 20h is).

STACK = 0x800

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	mov	bx, ax
	mov	dx, ds
	mov	ax, cs
	mov	ds, ax
	mov	es, ax
	cli
	mov	ss, ax
	mov	sp, __bss_end + STACK
	and	sp, 0xfffe
	sti
	cld
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	mov	word ptr [dos_entry_ax], bx
	mov	word ptr [edlin_seg], dx
	call	main
	jmp	edlin_exit

; u16 edlin_stack_top(void): the first offset past the stack.
	.section	.text.edlin_stack_top,"ax",@progbits
	.globl	edlin_stack_top
edlin_stack_top:
	mov	ax, __bss_end + STACK
	and	ax, 0xfffe
	ret

; AX as DOS passed it: AL is FFh if the first parameter named an invalid drive.
	.section	.bss.dos_entry_ax,"aw",@nobits
	.globl	dos_entry_ax
dos_entry_ax:
	.zero	2

; The segment of the program segment prefix: the original's segment (DG).
	.section	.bss.edlin_seg,"aw",@nobits
	.globl	edlin_seg
edlin_seg:
	.zero	2
