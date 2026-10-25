; libi8008rt: multiply and divide (abi.md sec.6).
; Convention (abi.md sec.3): i8 args in B, C; i16 args in BC, DE; result in
; B / BC. Every register and flag is clobbered. No frame; the 16-bit divide
; keeps its divisor and loop count in __rt_div (it is not reentrant, like
; every function on this machine).

; ---------------------------------------------------------------- multiply

; B * C -> B
	.section .text.__mulqi3,"ax",@progbits
	.globl __mulqi3
__mulqi3:
	ldi 0
1:	lac
	ora                 ; clears carry, Z if C == 0
	jtz 3f
	rar                 ; carry = low bit of C
	lca
	jfc 2f
	lad
	adb
	lda
2:	lab
	ada
	lba
	jmp 1b
3:	lbd
	ret

; BC * DE -> BC   (H:L is the accumulator)
	.section .text.__mulhi3,"ax",@progbits
	.globl __mulhi3
__mulhi3:
	lhi 0
	lli 0
1:	lad
	ore
	jtz 3f
	lad                 ; DE >>= 1, carry = old bit 0
	ora
	rar
	lda
	lae
	rar
	lea
	jfc 2f
	lal                 ; HL += BC
	adc
	lla
	lah
	acb
	lha
2:	lac                 ; BC <<= 1
	ada
	lca
	lab
	aca
	lba
	jmp 1b
3:	lbh
	lcl
	ret

; ---------------------------------------------------------------- 8-bit divide

; B / C: quotient in B, remainder in D. Restoring division, 8 steps (E).
	.section .text.__udivmodqi,"ax",@progbits
	.globl __udivmodqi
__udivmodqi:
	ldi 0
	lei 8
1:	lab                 ; dividend <<= 1, its top bit into the remainder
	ada
	lba
	lad
	ral
	lda
	jtc 3f              ; remainder >= 256 > divisor
	suc
	jtc 2f              ; remainder < divisor
	lda
	jmp 4f
3:	suc
	lda
4:	lab
	ori 1
	lba
2:	dce
	jfz 1b
	ret

	.section .text.__udivqi3,"ax",@progbits
	.globl __udivqi3
__udivqi3:
	jmp __udivmodqi

	.section .text.__umodqi3,"ax",@progbits
	.globl __umodqi3
__umodqi3:
	cal __udivmodqi
	lbd
	ret

; Signed: divide the magnitudes, then fix the signs. The quotient is negative
; when the signs differ; the remainder has the dividend's sign.
; __rt_sgn bit 7: quotient sign, bit 0: dividend sign.
	.section .text.__divmodqi,"ax",@progbits
__divmodqi:
	lab
	xrc
	ndi 0x80
	lea                 ; E = quotient sign (bit 7)
	lab
	ora
	jfs 1f
	lai 0               ; B = -B
	sub
	lba
	lae
	ori 1
	lea
1:	lac
	ora
	jfs 2f
	lai 0
	suc
	lca
2:	lhi %hi(__rt_sgn)
	lli %lo(__rt_sgn)
	lme
	jmp __udivmodqi

	.section .text.__divqi3,"ax",@progbits
	.globl __divqi3
__divqi3:
	cal __divmodqi
	lhi %hi(__rt_sgn)
	lli %lo(__rt_sgn)
	lam
	ora
	rfs
	lai 0
	sub
	lba
	ret

	.section .text.__modqi3,"ax",@progbits
	.globl __modqi3
__modqi3:
	cal __divmodqi
	lbd
	lhi %hi(__rt_sgn)
	lli %lo(__rt_sgn)
	lam
	ndi 1
	rtz
	lai 0
	sub
	lba
	ret

; ---------------------------------------------------------------- 16-bit divide

; BC / DE: quotient in BC, remainder in DE.
	.section .text.__udivmodhi,"ax",@progbits
	.globl __udivmodhi
__udivmodhi:
	lhi %hi(__rt_div)
	lli %lo(__rt_div)
	lme                 ; divisor low, high, then the step count
	inl
	lmd
	inl
	lmi 16
	ldi 0
	lei 0
1:	lac                 ; BC <<= 1, top bit into the remainder DE
	ada
	lca
	lab
	aca
	lba
	lae
	ral
	lea
	lad
	ral
	lda
	lli %lo(__rt_div)
	jtc 3f              ; remainder >= 65536 > divisor
	lae                 ; trial: DE - divisor
	sum
	inl
	lad
	sbm
	jtc 2f
3:	lli %lo(__rt_div)   ; DE -= divisor
	lae
	sum
	lea
	inl
	lad
	sbm
	lda
	lac
	ori 1
	lca
2:	lli %lo(__rt_div+2)
	lam
	sui 1
	lma
	jfz 1b
	ret

	.section .text.__udivhi3,"ax",@progbits
	.globl __udivhi3
__udivhi3:
	jmp __udivmodhi

	.section .text.__umodhi3,"ax",@progbits
	.globl __umodhi3
__umodhi3:
	cal __udivmodhi
	lbd
	lce
	ret

; Signed 16-bit, as for 8 bits; __rt_sgn holds the two signs.
	.section .text.__divmodhi,"ax",@progbits
__divmodhi:
	lab
	xrd
	ndi 0x80
	lhi %hi(__rt_sgn)
	lli %lo(__rt_sgn)
	lma
	lab
	ora
	jfs 1f
	lai 0               ; BC = -BC
	suc
	lca
	lai 0
	sbb
	lba
	lam
	ori 1
	lma
1:	lad
	ora
	jfs __udivmodhi
	lai 0               ; DE = -DE
	sue
	lea
	lai 0
	sbd
	lda
	jmp __udivmodhi

	.section .text.__divhi3,"ax",@progbits
	.globl __divhi3
__divhi3:
	cal __divmodhi
	lhi %hi(__rt_sgn)
	lli %lo(__rt_sgn)
	lam
	ora
	rfs
	lai 0
	suc
	lca
	lai 0
	sbb
	lba
	ret

	.section .text.__modhi3,"ax",@progbits
	.globl __modhi3
__modhi3:
	cal __divmodhi
	lbd
	lce
	lhi %hi(__rt_sgn)
	lli %lo(__rt_sgn)
	lam
	ndi 1
	rtz
	lai 0
	suc
	lca
	lai 0
	sbb
	lba
	ret

	.section .bss.__rt_div,"aw",@nobits
	.globl __rt_div
__rt_div:
	.zero 3
	.section .bss.__rt_sgn,"aw",@nobits
__rt_sgn:
	.zero 1
