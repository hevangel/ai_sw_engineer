; DEBUG's start, in place of runtime/crt0_com.s (spec D1).
;
; The rewrite is a load-high .EXE named DEBUG.COM (scripts/mkexe.py: MINALLOC = MAXALLOC = 0),
; so DOS gives it all memory but places the image at the top and leaves the program segment
; prefix's segment (DG, 0600h on the oracle machine) as the original's world. This code sets up
; the image's own segment for the Rust code (CS = DS = ES = SS = the image), clears .bss, keeps
; AX (the invalid-drive flag) and DG, and calls main. main never returns (DEBUG exits through
; INT 20h / the Q command).

STACK = 0x800

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	mov	bx, ax
	mov	dx, ds			; DG = the PSP's segment
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
	mov	word ptr [debug_dg], dx
	call	main
.Lhang:
	jmp	.Lhang

	.section	.bss.dos_entry_ax,"aw",@nobits
	.globl	dos_entry_ax
dos_entry_ax:
	.zero	2
	.section	.bss.debug_dg,"aw",@nobits
	.globl	debug_dg
debug_dg:
	.zero	2
