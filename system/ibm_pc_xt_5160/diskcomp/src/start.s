; DISKCOMP's start, in place of the shared runtime/crt0_com.s (spec D1).
;
; crt0_com.s puts the stack at the top of the program's segment (FFF0h). DISKCOMP's buffers
; start right after the program and run on for up to 42 tracks (189 KiB), across that address,
; so the stack must not be there: like the original, which keeps its stack in the PSP, the
; rewrite keeps it below the buffers, here right after .bss (STACK bytes, diskcomp_free its end).
; Otherwise as crt0_com.s: AX at entry kept (dos_entry_ax), .bss cleared, main called, INT 20h.

STACK = 0x400

	.section	.text.start,"ax",@progbits
	.globl	_start
_start:
	mov	bx, ax
	cld
	mov	sp, __bss_end + STACK
	and	sp, 0xfffe
	mov	di, __bss_start
	mov	cx, __bss_end
	sub	cx, di
	xor	al, al
	rep
	stosb
	mov	word ptr [dos_entry_ax], bx
	call	main
	int	0x20

; u16 diskcomp_free(void): the first offset past the stack, where the buffers may start.
	.section	.text.diskcomp_free,"ax",@progbits
	.globl	diskcomp_free
diskcomp_free:
	mov	ax, __bss_end + STACK
	ret

; AX as DOS passed it: AL (AH) is FFh if parameter 1 (2) named an invalid drive.
	.section	.bss.dos_entry_ax,"aw",@nobits
	.globl	dos_entry_ax
dos_entry_ax:
	.zero	2
