; SCELBAL's multi-byte carry primitives (the listing's ROTATL, ROTATR,
; COMPLM, ADDER, SUBBER) for the target: loops over page 01 of the RAM
; image (src/mem.rs, `basic_ram`) that keep the carry in the 8008's carry
; flag. src/fp.rs has the same routines in Rust for the host, and the
; contract of each. spec "Target".
;
; Normal calling convention (abi.md sec.3): 8-bit arguments in B, C, D;
; an 8-bit result in B; everything else may be clobbered. `n` = 0 means
; 256, as in the listing. INR/DCR and LAI/LLx leave the carry alone.

; basic_rotl(o B, n C, carry D) -> carry B: rotate n bytes left through the
; carry from the least significant byte at offset o.
	.section .text.basic_rotl,"ax",@progbits
	.globl basic_rotl
basic_rotl:
	lhi %hi(basic_ram)
	llb
	lad
	rar                 ; carry = D
1:	lam
	ral
	lma
	inl
	dcc
	jfz 1b
	lai 0
	ral
	lba
	ret

; basic_rotr(o B, n C, carry D) -> carry B: rotate right from the most
; significant byte at offset o downwards.
	.section .text.basic_rotr,"ax",@progbits
	.globl basic_rotr
basic_rotr:
	lhi %hi(basic_ram)
	llb
	lad
	rar
1:	lam
	rar
	lma
	dcl
	dcc
	jfz 1b
	lai 0
	ral
	lba
	ret

; basic_complm(o B, n C): negate n bytes from the least significant at o
; (0 - x, the borrow carried up).
	.section .text.basic_complm,"ax",@progbits
	.globl basic_complm
basic_complm:
	lhi %hi(basic_ram)
	llb
	xra                 ; no borrow in
1:	lai 0
	sbm
	lma
	inl
	dcc
	jfz 1b
	ret

; basic_adder(a B, b C, n D): bytes at b += bytes at a.
	.section .text.basic_adder,"ax",@progbits
	.globl basic_adder
basic_adder:
	lhi %hi(basic_ram)
	xra
1:	llb
	lam
	llc
	acm
	lma
	inb
	inc
	dcd
	jfz 1b
	ret

; basic_subber(a B, b C, n D): bytes at b = bytes at a - bytes at b.
	.section .text.basic_subber,"ax",@progbits
	.globl basic_subber
basic_subber:
	lhi %hi(basic_ram)
	xra
1:	llb
	lam
	llc
	sbm
	lma
	inb
	inc
	dcd
	jfz 1b
	ret
