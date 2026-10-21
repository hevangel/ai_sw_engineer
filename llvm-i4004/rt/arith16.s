; libi4004rt -- 16-bit arithmetic helpers (__mulhi3 / __udivhi3 / __umodhi3)
; ============================================================================
; Contract (abi.md sec.4.1): i16 a in D1 = P2:P3, i16 b in D2 = P4:P5, i16
; result in D1 = P2:P3.
;   D1 = P2:P3 : low byte P2 = R4(hi):R5(lo), high byte P3 = R6(hi):R7(lo)
;        value nibbles LSB..MSB:  R5, R4, R7, R6
;   D2 = P4:P5 : low byte P4 = R8(hi):R9(lo), high byte P5 = R10(hi):R11(lo)
;        value nibbles LSB..MSB:  R9, R8, R11, R10
; Multi-nibble subtract (verified on chips, arith.s/README): CLC once, SUB low
; nibble, CMC before each higher nibble; CY=1 => no borrow (minuend >= subtrahend).
; Leaf/one-level helpers; touch only the caller-clobbered set plus, in the
; divider, P6 which is SAVED/RESTORED via the soft stack. Execution-tested on
; external/chips vs a host oracle (rt/tests, --wide).

; ---------------------------------------------------------------------------
; __mulhi3 : (a:D1) * (b:D2) -> low 16 bits in D1.
;   result = 0; while a != 0 { if a&1 result += b; a >>= 1; b <<= 1; }
;   No iteration counter: the low-16 product is complete once a is exhausted.
;   result in P0:P1 (nibbles R0 R1 R2 R3 = bits[15:12][11:8][7:4][3:0]);
;   a shifts right in D1, b shifts left in D2.
; ---------------------------------------------------------------------------
	.section .text.__mulhi3,"ax"
	.globl	__mulhi3
__mulhi3:
	ldm	0
	xch	r0
	ldm	0
	xch	r1
	ldm	0
	xch	r2
	ldm	0
	xch	r3			; result (P0:P1) = 0
.Lmul16_loop:
	ld	r5
	jcn	nz, .Lmul16_go
	ld	r4
	jcn	nz, .Lmul16_go
	ld	r7
	jcn	nz, .Lmul16_go
	ld	r6
	jcn	nz, .Lmul16_go
	jun	.Lmul16_done		; a == 0 -> done
.Lmul16_go:
	ld	r5
	rar				; CY <- a bit0
	jcn	nc, .Lmul16_shift
	; result += b   (LSB..MSB: res R3,R2,R1,R0 ; b R9,R8,R11,R10)
	clc
	ld	r3
	add	r9
	xch	r3
	ld	r2
	add	r8
	xch	r2
	ld	r1
	add	r11
	xch	r1
	ld	r0
	add	r10
	xch	r0
.Lmul16_shift:
	clc
	ld	r6
	rar
	xch	r6
	ld	r7
	rar
	xch	r7
	ld	r4
	rar
	xch	r4
	ld	r5
	rar
	xch	r5			; a >>= 1
	clc
	ld	r9
	ral
	xch	r9
	ld	r8
	ral
	xch	r8
	ld	r11
	ral
	xch	r11
	ld	r10
	ral
	xch	r10			; b <<= 1
	jun	.Lmul16_loop
.Lmul16_done:
	ld	r2
	xch	r4
	ld	r3
	xch	r5			; D1 low byte  = res[7:4]:res[3:0]
	ld	r0
	xch	r6
	ld	r1
	xch	r7			; D1 high byte = res[15:12]:res[11:8]
	bbl	0

; ---------------------------------------------------------------------------
; __udivmod16 (internal): numerator in D1 (P2:P3), divisor in D2 (P4:P5),
;   divisor assumed nonzero. Restoring shift-left division, 16 iterations.
;   The {rem : num} 32-bit shift register is rem(P0:P1) : num(D1=P2:P3); the
;   quotient is shifted into num's low bits as num shifts out the top, so num
;   and quotient share D1 (standard in-place long division). On return:
;     quotient in D1 (P2:P3), remainder in P0:P1
;       (R0 R1 R2 R3 = rem bits[15:12][11:8][7:4][3:0]).
;   The 16-iteration counter uses R12 (P6 low nibble, callee-saved): its prior
;   value is pushed to the soft stack on entry and popped before BBL, so the
;   ABI's callee-saved guarantee on P6 holds. Trial subtraction is done as a
;   COMPARE first (SUB chain discarding results, keeping only CY) so no extra
;   scratch is needed; on rem>=divisor the real subtract runs in place.
; ---------------------------------------------------------------------------
	.section .text.__udivmod16,"ax"
	.globl	__udivmod16
__udivmod16:
	; Save old R12 (P6 low, callee-saved) at SSP using ONLY ACC + P7, so none
	; of rem (P0:P1 = R0..R3) or num/quot/divisor is disturbed. (The library
	; __push_byte/__pop_byte would clobber P1 = R2:R3, which here holds rem's
	; low byte -- that corrupted __umodhi3's result.) SSP (P7) grows up.
	src	p7
	ld	r12
	wrm				; RAM[SSP] = old R12
	inc	r15			; SSP.char++  (SSP now points past the saved nibble)
	; rem = 0
	ldm	0
	xch	r0
	ldm	0
	xch	r1
	ldm	0
	xch	r2
	ldm	0
	xch	r3
	; counter R12 = 0; ISZ increments to 0 after 16 steps (16 - 16 mod 16 = 0;
	; seed with 0 so ISZ wraps after 16 iterations).
	ldm	0
	xch	r12
.Ldiv16_loop:
	; {rem:num} <<= 1 (LSB..MSB: num R5,R4,R7,R6 ; rem R3,R2,R1,R0)
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2
	ld	r1
	ral
	xch	r1
	ld	r0
	ral
	xch	r0			; rem high nibble; CY out discarded
	; COMPARE rem - divisor (discard differences, keep CY). CY=1 => rem>=div.
	clc
	ld	r3
	sub	r9
	cmc
	ld	r2
	sub	r8
	cmc
	ld	r1
	sub	r11
	cmc
	ld	r0
	sub	r10			; CY=1 => no borrow => rem >= divisor
	jcn	nc, .Ldiv16_next	; borrow => rem < divisor => quotient bit 0
	; rem >= divisor: rem -= divisor (in place) and set quotient bit0 (num R5 b0)
	clc
	ld	r3
	sub	r9
	xch	r3
	cmc
	ld	r2
	sub	r8
	xch	r2
	cmc
	ld	r1
	sub	r11
	xch	r1
	cmc
	ld	r0
	sub	r10
	xch	r0			; rem -= divisor
	ld	r5
	iac
	xch	r5			; quotient bit0 = 1 (num.lo bit0 was 0 after shift)
.Ldiv16_next:
	isz	r12, .Ldiv16_loop	; 16 iterations
	; restore old R12 (P6 low) from the soft stack, using ONLY ACC + P7 so rem
	; (R0..R3) is preserved. SSP currently points one past the saved nibble.
	ld	r15
	dac
	xch	r15			; SSP.char-- (back to the saved nibble)
	src	p7
	rdm				; ACC = old R12
	xch	r12			; R12 = old value
	bbl	0

; ---------------------------------------------------------------------------
; __udivhi3 : (a:D1) / (b:D2) -> quotient in D1.
; ---------------------------------------------------------------------------
	.section .text.__udivhi3,"ax"
	.globl	__udivhi3
__udivhi3:
	jms	__udivmod16		; quotient already in D1 (P2:P3)
	bbl	0

; ---------------------------------------------------------------------------
; __umodhi3 : (a:D1) % (b:D2) -> remainder in D1.
;   __udivmod16 leaves the remainder in P0:P1; move it into D1 (P2:P3).
; ---------------------------------------------------------------------------
	.section .text.__umodhi3,"ax"
	.globl	__umodhi3
__umodhi3:
	jms	__udivmod16
	; rem P0:P1 (R0 R1 R2 R3 = [15:12][11:8][7:4][3:0]) -> D1 (P2:P3)
	; D1 low byte  = R4:R5 = rem[7:4]:rem[3:0] = R2:R3
	; D1 high byte = R6:R7 = rem[15:12]:rem[11:8] = R0:R1
	ld	r2
	xch	r4
	ld	r3
	xch	r5
	ld	r0
	xch	r6
	ld	r1
	xch	r7
	bbl	0
