; libi4004rt -- integer arithmetic helpers
; ============================================================================
; ABI: llvm-i4004/spec/abi.md. Args in pairs P1,P2,P3; i8 return in P1 (R2:R3).
; A pair Pn holds {R(2n)=high nibble, R(2n+1)=low nibble} (isa.md: even reg is
; high). An 8-bit value = hi*16 + lo. Subtract convention isa.md sec.7: SUB =
; ACC + ~R + ~CY, so CY=1 means "no borrow"; a subtract chain sets CY=1 first.
;
; These helpers use only caller-clobbered index registers R0-R7 (pairs
; P0,P1,P2,P3) as scratch, so a caller's callee-saved P4-P6 and the SSP P7
; survive a runtime call untouched. Call depth is at most 2 (a div/mod wrapper
; calling __udivmod), well inside the 3-level hardware stack; no software stack.
;
; Each helper sits in its OWN .text.<name> section so the image packer's
; --pad-pages keeps it whole inside one 256-byte 4001 page -- the in-helper JCN
; branches are page-local (isa.md sec.9) and MUST NOT cross a page.
;
; Register map (all caller-clobbered):
;   R2:R3 = P1 = arg0 / result   R4:R5 = P2 = arg1
;   R6:R7 = P3 = working accumulator   R0 = loop counter   R1 = scratch

; ---------------------------------------------------------------------------
; __mulqi3 : (i8 a=P1) * (i8 b=P2) -> i8 (P1). Low 8 bits of the product.
; result=0; repeat 8x { if(a&1) result+=b; a>>=1; b<<=1; }. Clobbers R0-R7.
; ---------------------------------------------------------------------------
	.section .text.__mulqi3,"ax"
	.globl	__mulqi3
__mulqi3:
	ldm	0
	xch	r6			; result.hi = 0
	ldm	0
	xch	r7			; result.lo = 0
	ldm	8
	xch	r0			; loop 8 times (ISZ wraps 8->0)
.Lmul_loop:
	ld	r3
	rar				; CY <- a bit0
	jcn	nc, .Lmul_shift		; NC: CY==0 -> skip add
	clc
	ld	r7
	add	r5
	xch	r7			; result.lo += b.lo
	ld	r6
	add	r4
	xch	r6			; result.hi += b.hi + carry
.Lmul_shift:
	clc
	ld	r2
	rar
	xch	r2			; a.hi >>= 1
	ld	r3
	rar
	xch	r3			; a.lo >>= 1 (bit3 <- carry out of a.hi)
	clc
	ld	r5
	ral
	xch	r5			; b.lo <<= 1
	ld	r4
	ral
	xch	r4			; b.hi <<= 1 (bit0 <- carry out of b.lo)
	isz	r0, .Lmul_loop
	ld	r6
	xch	r2
	ld	r7
	xch	r3			; move result P3 -> P1
	bbl	0

; ---------------------------------------------------------------------------
; __udivmod (internal): P1 = numerator, P2 = divisor (nonzero).
;   Returns quotient in P1 (R2:R3), remainder in P3 (R6:R7).
;   Restoring shift-left division, 8 iterations. Leaf. Clobbers R0-R7.
; ---------------------------------------------------------------------------
	.section .text.__udivmod,"ax"
	.globl	__udivmod
__udivmod:
	ldm	0
	xch	r6			; rem.hi = 0
	ldm	0
	xch	r7			; rem.lo = 0
	ldm	8
	xch	r0			; 8 iterations
.Ldiv_loop:
	; 16-bit left shift {rem(R6:R7):num(R2:R3)} by 1; new bit0 = 0.
	clc
	ld	r3
	ral
	xch	r3			; num.lo <<= 1
	ld	r2
	ral
	xch	r2			; num.hi <<= 1
	ld	r7
	ral
	xch	r7			; rem.lo <<= 1
	ld	r6
	ral
	xch	r6			; rem.hi <<= 1
	; trial rem - divisor (8-bit), tentative result in R1(lo)/ACC(hi).
	; MCS-4 multi-nibble subtract (verified on the chips model): CLC once, SUB
	; the low nibble, then CMC before the high nibble's SUB so ~CY propagates
	; the borrow correctly. SUB computes ACC + ~R + ~CY (isa.md sec.7); without
	; the CMC the borrow carries the wrong sense and the high nibble is off by
	; one. Final CY=1 => no borrow (rem >= divisor) => commit.
	clc
	ld	r7
	sub	r5			; rem.lo - div.lo
	xch	r1			; tentative rem.lo
	cmc				; propagate borrow into the high nibble
	ld	r6
	sub	r4			; rem.hi - div.hi - borrow; CY=1 => rem >= divisor
	jcn	nc, .Ldiv_next		; CY==0 -> borrow -> divisor>rem -> restore
	xch	r6			; commit rem.hi
	ld	r1
	xch	r7			; commit rem.lo
	ld	r3
	iac				; quotient bit0 = 1 (num.lo bit0 was 0)
	xch	r3
.Ldiv_next:
	isz	r0, .Ldiv_loop
	bbl	0

	.section .text.__udivqi3,"ax"
	.globl	__udivqi3
__udivqi3:
	jms	__udivmod
	bbl	0			; quotient already in P1

	.section .text.__umodqi3,"ax"
	.globl	__umodqi3
__umodqi3:
	jms	__udivmod
	ld	r6
	xch	r2
	ld	r7
	xch	r3			; remainder P3 -> P1
	bbl	0

; ---------------------------------------------------------------------------
; __negqi2 : i8 P1 = -P1 (two's complement). Leaf. Clobbers ACC,CY.
;   two's complement = (~x)+1, nibble-serial with carry.
; ---------------------------------------------------------------------------
	.section .text.__negqi2,"ax"
	.globl	__negqi2
__negqi2:
	ld	r3
	cma
	iac				; lo = ~lo + 1 ; CY = carry out
	xch	r3
	ld	r2
	cma			; hi = ~hi
	jcn	nc, .Lneg_done		; no carry into hi
	iac				; hi += 1
.Lneg_done:
	xch	r2
	bbl	0
