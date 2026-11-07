; The BIOS call MORE makes that the shared ibm_pc_dos crate has no function for (spec D3).
; Arguments and results follow llvm-i8086/spec/abi.md D3; BX, SI, DI and BP are preserved,
; and so are DS and ES.

; u8 more_video_columns(void): INT 10h AH=0Fh (current video state) returns AL = the mode,
; AH = the number of text columns, BH = the active page; the result is AH.
	.section	.text.more_video_columns,"ax",@progbits
	.globl	more_video_columns
more_video_columns:
	push	bx
	push	bp
	mov	ah, 0x0f
	int	0x10
	mov	al, ah
	xor	ah, ah
	pop	bp
	pop	bx
	ret
