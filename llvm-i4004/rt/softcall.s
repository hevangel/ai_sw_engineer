; libi4004rt — software-return-stack call convention (abi.md 4.4.1).
; __push_ret: leaf helper (JMS/BBL, hardware depth 1) — push the 3-nibble return
;   token already staged in R3(off lo), R2(off hi), R4(page) onto SSP (P7), SSP+=3.
;
; __soft_ret, the 16-entry __srtab, and the per-page `jin p0` stubs are
; SYNTHESIZED BY THE PACKER (i4004_pack.py), because only the linker knows the
; final page layout and __srtab's address — see abi.md 4.4.2. This file carries
; only the layout-independent push helper.

	.section .text.__push_ret,"ax"
	.globl __push_ret
__push_ret:
	src	p7
	ld	r3
	wrm			; [SSP+0] = return offset low
	inc	r15
	src	p7
	ld	r2
	wrm			; [SSP+1] = return offset high
	inc	r15
	src	p7
	ld	r4
	wrm			; [SSP+2] = return page
	inc	r15
	bbl	0

