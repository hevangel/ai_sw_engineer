; libi4004rt -- variable-amount 8-bit shifts
; ============================================================================
; __ashlqi3 : (i8 val=P1) << (i8 n=P2) -> i8 (P1)   logical left
; __lshrqi3 : (i8 val=P1) >> (i8 n=P2) -> i8 (P1)   logical right (zero-fill)
; __ashrqi3 : (i8 val=P1) >> (i8 n=P2) -> i8 (P1)   arithmetic right (sign-fill)
;
; n is taken mod ... no: libgcc semantics are that a shift count >= width is UB.
; We clamp: n >= 8 yields 0 for the logical shifts and 0/0xFF (sign) for ashr,
; which is the sane, test-checkable behaviour. The count is in P2 low byte;
; only the low 4+ bits matter.  Each shift is one bit at a time, counted by an
; index register, looping with ISZ (page-local branch).
;
; Register map (caller-clobbered): R2:R3 = val/result, R4:R5 = n (R5 lo used),
; R0 = bit counter.  Leaf helpers, no stack.  See README for sizes.

; ---------------------------------------------------------------------------
; __ashlqi3 : logical/arith left shift by n (same thing). Shift val left n bits.
; ---------------------------------------------------------------------------
	.section .text.__ashlqi3,"ax"
	.globl	__ashlqi3
__ashlqi3:
	; if n == 0, return val unchanged
	ld	r5
	jcn	z, .Lshl_chkhi		; low nibble zero? check high nibble of n
	jun	.Lshl_go
.Lshl_chkhi:
	ld	r4
	jcn	z, .Lshl_done		; n == 0 -> done
.Lshl_go:
	; counter = 16 - n  so ISZ wraps to 0 after n increments. For n>=8 the
	; result is 0; a one-bit-at-a-time loop naturally zeroes it (<=15 shifts).
	; Build -n in R0: R0 = 0 - n.lo (ignore n.hi; n>=16 clamps via wrap).
	ldm	0
	sub	r5			; ACC = 0 - n  (CY ignored); = 16-n mod 16
	xch	r0			; R0 = 16 - n (mod 16); if n=0 handled above
.Lshl_loop:
	clc
	ld	r3
	ral
	xch	r3			; val.lo <<= 1
	ld	r2
	ral
	xch	r2			; val.hi <<= 1 (bit0 <- carry out of lo)
	isz	r0, .Lshl_loop
.Lshl_done:
	bbl	0

; ---------------------------------------------------------------------------
; __lshrqi3 : logical right shift by n (zero fill).
; ---------------------------------------------------------------------------
	.section .text.__lshrqi3,"ax"
	.globl	__lshrqi3
__lshrqi3:
	ld	r5
	jcn	z, .Llsr_chkhi
	jun	.Llsr_go
.Llsr_chkhi:
	ld	r4
	jcn	z, .Llsr_done
.Llsr_go:
	ldm	0
	sub	r5
	xch	r0			; R0 = 16 - n
.Llsr_loop:
	clc				; zero fill from the top
	ld	r2
	rar
	xch	r2			; val.hi >>= 1 (bit3 <- 0)
	ld	r3
	rar
	xch	r3			; val.lo >>= 1 (bit3 <- carry out of hi)
	isz	r0, .Llsr_loop
.Llsr_done:
	bbl	0

; ---------------------------------------------------------------------------
; __ashrqi3 : arithmetic right shift by n (sign fill from bit7 = val.hi bit3).
; ---------------------------------------------------------------------------
	.section .text.__ashrqi3,"ax"
	.globl	__ashrqi3
__ashrqi3:
	ld	r5
	jcn	z, .Lasr_chkhi
	jun	.Lasr_go
.Lasr_chkhi:
	ld	r4
	jcn	z, .Lasr_done
.Lasr_go:
	ldm	0
	sub	r5
	xch	r0			; R0 = 16 - n
.Lasr_loop:
	; set CY = sign bit (val.hi bit3) so RAR fills it back in.
	ld	r2
	ral				; CY <- val.hi bit3 (ACC trashed, reloaded)
	ld	r2
	rar				; val.hi >>= 1, bit3 <- CY (sign) ; CY <- old bit0
	xch	r2
	ld	r3
	rar				; val.lo >>= 1, bit3 <- carry out of hi
	xch	r3
	isz	r0, .Lasr_loop
.Lasr_done:
	bbl	0
