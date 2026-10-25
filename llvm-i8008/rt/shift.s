; libi8008rt: shifts by a variable amount (abi.md sec.6).
; i8: value in B, count in C -> B.  i16: value in BC, count in D -> BC.
; A count of 0 returns the value; counts >= the width are undefined in LLVM IR
; and simply shift that many times here.

	.section .text.__ashlqi3,"ax",@progbits
	.globl __ashlqi3
__ashlqi3:
1:	lac
	ora
	rtz
	lab
	ada
	lba
	dcc
	jmp 1b

	.section .text.__lshrqi3,"ax",@progbits
	.globl __lshrqi3
__lshrqi3:
1:	lac
	ora
	rtz
	lab
	ora                 ; clear carry
	rar
	lba
	dcc
	jmp 1b

	.section .text.__ashrqi3,"ax",@progbits
	.globl __ashrqi3
__ashrqi3:
1:	lac
	ora
	rtz
	lab
	rlc                 ; carry = sign; rrc restores A
	rrc
	rar
	lba
	dcc
	jmp 1b

	.section .text.__ashlhi3,"ax",@progbits
	.globl __ashlhi3
__ashlhi3:
1:	lad
	ora
	rtz
	lac
	ada
	lca
	lab
	aca
	lba
	dcd
	jmp 1b

	.section .text.__lshrhi3,"ax",@progbits
	.globl __lshrhi3
__lshrhi3:
1:	lad
	ora
	rtz
	lab
	ora
	rar
	lba
	lac
	rar
	lca
	dcd
	jmp 1b

	.section .text.__ashrhi3,"ax",@progbits
	.globl __ashrhi3
__ashrhi3:
1:	lad
	ora
	rtz
	lab
	rlc
	rrc
	rar
	lba
	lac
	rar
	lca
	dcd
	jmp 1b
