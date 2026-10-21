; libi4004rt -- variable-amount 16-bit shifts (__ashlhi3/__lshrhi3/__ashrhi3)
; ============================================================================
; Contract (abi.md sec.4.1): i16 value in D1 = P2:P3, i16 count in D2 = P4:P5,
; result i16 in D1 = P2:P3.
;   D1 low byte  = P2 = R4(hi):R5(lo)   -> value bits [7:4]=R4, [3:0]=R5
;   D1 high byte = P3 = R6(hi):R7(lo)   -> value bits [15:12]=R6, [11:8]=R7
;   D2 count low byte = P4 = R8(hi):R9(lo); only the low bits of the count
;   matter. A count >= 16 yields 0 (logical) / 0 or -1 (ashr sign-fill), which
;   the one-bit loop produces naturally (at most 15 iterations before all bits
;   are gone), so we loop exactly `n` times with n taken from the low byte and
;   clamped to 16 when the high byte or a carry says n >= 16.
;
; The 16-bit value is four nibbles R5(lo) R4 R7 R6(hi). Each iteration does a
; 16-bit 1-bit shift across them with CY chaining. Counter in R0 counts DOWN:
; R0 = 16 - (n mod 16); ISZ wraps R0 to 0 after exactly that many steps, which
; for n in 1..15 is n iterations; for n==0 we skip; for n>=16 we force 0 result.
; Leaf helpers, no stack. Scratch: R0 (counter), R1. Execution-tested on chips.

; --- helper macro-ish: detect n==0 (return unchanged) or n>=16 (return 0/sign).
;     n low byte = R8:R9, high byte = R10:R11 (D2). n>=16 iff any of R10,R11,R8
;     is nonzero (since the low nibble R9 alone is < 16).

; ---------------------------------------------------------------------------
; __ashlhi3 : D1 << n  (logical left; fill 0 from the bottom)
; ---------------------------------------------------------------------------
	.section .text.__ashlhi3,"ax"
	.globl	__ashlhi3
__ashlhi3:
	; n >= 16 ? (R8 | R10 | R11 != 0)  -> result 0
	ld	r8
	jcn	nz, .Lshl16_zero
	ld	r10
	jcn	nz, .Lshl16_zero
	ld	r11
	jcn	nz, .Lshl16_zero
	; n (low nibble R9) == 0 ? -> return unchanged
	ld	r9
	jcn	z, .Lshl16_done
	; R0 = 16 - n   (0 - n mod 16)
	ldm	0
	sub	r9
	xch	r0
.Lshl16_loop:
	clc
	ld	r5
	ral
	xch	r5			; bits[3:0]  <<1
	ld	r4
	ral
	xch	r4			; bits[7:4]  <<1
	ld	r7
	ral
	xch	r7			; bits[11:8] <<1
	ld	r6
	ral
	xch	r6			; bits[15:12]<<1
	isz	r0, .Lshl16_loop
.Lshl16_done:
	bbl	0
.Lshl16_zero:
	ldm	0
	xch	r4
	ldm	0
	xch	r5
	ldm	0
	xch	r6
	ldm	0
	xch	r7
	bbl	0

; ---------------------------------------------------------------------------
; __lshrhi3 : D1 >> n  (logical right; zero fill from the top)
; ---------------------------------------------------------------------------
	.section .text.__lshrhi3,"ax"
	.globl	__lshrhi3
__lshrhi3:
	ld	r8
	jcn	nz, .Llsr16_zero
	ld	r10
	jcn	nz, .Llsr16_zero
	ld	r11
	jcn	nz, .Llsr16_zero
	ld	r9
	jcn	z, .Llsr16_done
	ldm	0
	sub	r9
	xch	r0
.Llsr16_loop:
	clc				; zero-fill
	ld	r6
	rar
	xch	r6			; bits[15:12] >>1
	ld	r7
	rar
	xch	r7			; bits[11:8]  >>1
	ld	r4
	rar
	xch	r4			; bits[7:4]   >>1
	ld	r5
	rar
	xch	r5			; bits[3:0]   >>1
	isz	r0, .Llsr16_loop
.Llsr16_done:
	bbl	0
.Llsr16_zero:
	ldm	0
	xch	r4
	ldm	0
	xch	r5
	ldm	0
	xch	r6
	ldm	0
	xch	r7
	bbl	0

; ---------------------------------------------------------------------------
; __ashrhi3 : D1 >> n  (arithmetic right; sign fill from bit15 = R6 bit3)
; ---------------------------------------------------------------------------
	.section .text.__ashrhi3,"ax"
	.globl	__ashrhi3
__ashrhi3:
	; n >= 16 -> result is 0 (sign 0) or 0xFFFF (sign 1): sign-fill all.
	ld	r8
	jcn	nz, .Lasr16_sat
	ld	r10
	jcn	nz, .Lasr16_sat
	ld	r11
	jcn	nz, .Lasr16_sat
	ld	r9
	jcn	z, .Lasr16_done
	ldm	0
	sub	r9
	xch	r0
.Lasr16_loop:
	; CY <- sign bit (R6 bit3) so RAR fills it back in at the top.
	ld	r6
	ral				; CY <- R6 bit3 (ACC trashed)
	ld	r6
	rar				; bits[15:12] >>1, bit3 <- sign
	xch	r6
	ld	r7
	rar
	xch	r7			; bits[11:8] >>1
	ld	r4
	rar
	xch	r4			; bits[7:4]  >>1
	ld	r5
	rar
	xch	r5			; bits[3:0]  >>1
	isz	r0, .Lasr16_loop
.Lasr16_done:
	bbl	0
.Lasr16_sat:
	; saturate: if sign bit (R6 bit3) set -> all 0xF, else all 0.
	ld	r6
	ral				; CY <- sign
	jcn	nc, .Lasr16_zero
	ldm	15
	xch	r4
	ldm	15
	xch	r5
	ldm	15
	xch	r6
	ldm	15
	xch	r7
	bbl	0
.Lasr16_zero:
	ldm	0
	xch	r4
	ldm	0
	xch	r5
	ldm	0
	xch	r6
	ldm	0
	xch	r7
	bbl	0
