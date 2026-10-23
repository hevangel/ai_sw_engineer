; Busicom 141-PF firmware -- BCD register primitives (issue #18, B2)
; ============================================================================
; The digit loops of `src/bcd.rs` on the target. A number lives in a 4002
; register (16 main characters = digits 0..15, least significant first; S0 =
; sign, S1:S2 = scale) and is passed by HANDLE: the SRC address of character
; 0. Buffers are runs of characters at any SRC address (the address is linear
; across registers, so `ISZ lo, 1f; INC hi; 1:` advances an 8-bit pointer).
;
; Every routine is a LEAF (no JMS): the firmware's call graph must fit the
; 4004's 3-level hardware stack, and Rust code calls these at depth <= 2.
; They follow the compiled ABI (llvm-i4004/spec/abi.md sec.4): u8 arguments in
; P1 (R2:R3), P2 (R4:R5), P3 (R6:R7), u8 result in P1. The `__leaf_*` routines
; clobber ACC, CY and P0-P3 ONLY: llc keeps P4:P5 live across a call to a
; `__leaf_*` symbol (abi.md 4.3.1), so they must never touch R8-R11.
; None touches P6 (callee-saved) or P7 (reserved).
;
; Decimal arithmetic uses the MCS-4 idioms: ADM + DAA adds a digit with carry,
; TCS + SBM + ADM + DAA subtracts with borrow (CY = 1 means "no borrow"), and a
; bare SBM chain compares without writing. Each routine is checked against its
; Rust twin in `src/bcd.rs` on external/chips (llvm-i4004/scripts/
; exec_num.py, issue #18).
;
; Buffer lengths are passed NEGATED (`len.wrapping_neg()`, folded by rustc):
; a loop runs exactly `len` times as `ISZ cnt.lo, loop; ISZ cnt.hi, loop`.

; ---------------------------------------------------------------------------
; __leaf_n_clear(a): digits, sign and scale of register `a` = 0.
	.section .text.__leaf_n_clear,"ax"
	.globl	__leaf_n_clear
__leaf_n_clear:
	clb
1:	src	p1
	wrm
	isz	r3, 1b		; 16 characters (R3 wraps back to 0)
	src	p1
	wr0
	wr1
	wr2
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_copy(dst, src): digits, sign and scale.
; __leaf_n_cpyd(dst, src): digits only (src may be a buffer's register-aligned
; start; used for results assembled in scratch).
; __leaf_n_cp(dst | src >> 4): __leaf_n_copy with both register numbers in one
; byte (a constant pair is one FIM instead of two).
	.section .text.__leaf_n_copy,"ax"
	.globl	__leaf_n_copy
	.globl	__leaf_n_cpyd
	.globl	__leaf_n_cp
__leaf_n_cp:
	ld	r3
	xch	r4
	ldm	0
	xch	r5
	ldm	0
	xch	r3
__leaf_n_copy:
	src	p2
	rd0
	src	p1
	wr0
	src	p2
	rd1
	src	p1
	wr1
	src	p2
	rd2
	src	p1
	wr2
__leaf_n_cpyd:
1:	src	p2
	rdm
	src	p1
	wrm
	inc	r5
	isz	r3, 1b
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_digits(a) -> number of significant digits (0..16).
	.section .text.__leaf_n_digits,"ax"
	.globl	__leaf_n_digits
__leaf_n_digits:
	ldm	15
	xch	r3		; from digit 15 down
1:	src	p1
	rdm
	jcn	nz, 3f
	ld	r3
	jcn	z, 2f		; digit 0 is zero too: no digits
	dac
	xch	r3
	jun	1b
2:	fim	p1, 0
	bbl	0
3:	ld	r3
	iac			; index + 1; CY on 16
	xch	r3
	tcc
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_cmp(a, b) -> 0 (|a| < |b|), 1 (equal), 2 (|a| > |b|). Most significant
; digit first; `CLC; SBM` gives a - b with CY = 1 iff a >= b.
	.section .text.__leaf_n_cmp,"ax"
	.globl	__leaf_n_cmp
__leaf_n_cmp:
	ldm	15
	xch	r3
	ldm	15
	xch	r5
1:	clc
	src	p1
	rdm
	src	p2
	sbm
	jcn	nz, 3f
	ld	r3
	jcn	z, 2f
	dac
	xch	r3
	ld	r5
	dac
	xch	r5
	jun	1b
2:	fim	p1, 1		; all digits equal
	bbl	0
3:	fim	p1, 2
	jcn	c, 4f		; no borrow and not equal: a > b
	fim	p1, 0
4:	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_add16(a, b) -> carry out: a += b over 16 BCD digits.
	.section .text.__leaf_n_add16,"ax"
	.globl	__leaf_n_add16
__leaf_n_add16:
	clc
1:	src	p2
	rdm
	src	p1
	adm
	daa
	wrm
	inc	r5
	isz	r3, 1b
	tcc
	xch	r3		; R3 = carry (R3 was back at 0)
	ldm	0
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_sub16(a, b): a -= b over 16 BCD digits (caller guarantees a >= b).
	.section .text.__leaf_n_sub16,"ax"
	.globl	__leaf_n_sub16
__leaf_n_sub16:
	stc			; no borrow
1:	tcs			; ACC = 10 (no borrow) or 9; CY = 0
	src	p2
	sbm			; - b
	clc
	src	p1
	adm			; + a
	daa			; >= 10: digit, CY = 1; else digit + 10 kept, CY = 0
	wrm
	inc	r5
	isz	r3, 1b
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_inc(a) -> carry out of digit 15: a += 1 (one unit in the last place).
	.section .text.__leaf_n_inc,"ax"
	.globl	__leaf_n_inc
__leaf_n_inc:
	stc
1:	src	p1
	ldm	0
	adm
	daa
	wrm
	jcn	nc, 2f		; carry absorbed
	isz	r3, 1b
	fim	p1, 1
	bbl	0
2:	fim	p1, 0
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_shl(a, k) -> 1 if shifted, 0 (unchanged) if digits(a) + k > 16:
; a *= 10^k (k is a byte).
	.section .text.__leaf_n_shl,"ax"
	.globl	__leaf_n_shl
__leaf_n_shl:
	ld	r4		; k >= 17 (k.hi >= 2, or k.hi == 1 and k.lo != 0) fails
	jcn	z, 1f
	ld	r5
	jcn	nz, 7f
	ld	r4
	dac
	jcn	nz, 7f
1:	ldm	15		; top nonzero digit index t
	xch	r3
2:	src	p1
	rdm
	jcn	nz, 3f
	ld	r3
	jcn	z, 6f		; zero register: shifting changes nothing
	dac
	xch	r3
	jun	2b
3:	ld	r4		; k == 16 with a digit present: fails
	jcn	nz, 7f
	clc
	ld	r3
	add	r5		; t + k >= 16 fails
	jcn	c, 7f
	ld	r5
	jcn	z, 6f		; k == 0
	clb
	sub	r5
	xch	r6		; R6 = 16 - k: ISZ runs k passes
4:	ldm	0
	xch	r3
	ldm	0
	xch	r7		; carry digit
5:	src	p1		; d[i] <-> carry, ascending: shift up one
	rdm
	xch	r7
	wrm
	isz	r3, 5b
	isz	r6, 4b
6:	fim	p1, 1
	bbl	0
7:	fim	p1, 0
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_shr(a, k) -> the most significant dropped digit (0 when k > 16, or k
; == 0): a /= 10^k, truncating (k is a byte).
	.section .text.__leaf_n_shr,"ax"
	.globl	__leaf_n_shr
__leaf_n_shr:
	ld	r4
	jcn	nz, 1f
	ld	r5
	jcn	z, 6f		; k == 0
	clb
	sub	r5
	xch	r6		; R6 = 16 - k
	jun	2f
1:	ld	r5		; k >= 16: exactly 16 iff k.hi == 1 and k.lo == 0
	jcn	nz, 5f
	ld	r4
	dac
	jcn	nz, 5f
	ldm	0
	xch	r6		; 16 passes
2:	ldm	15
	xch	r3
	ldm	0
	xch	r7		; carry digit, from the top
3:	src	p1		; d[i] <-> carry, descending: shift down one
	rdm
	xch	r7
	wrm
	ld	r3
	jcn	z, 4f
	dac
	xch	r3
	jun	3b
4:	isz	r6, 2b
	ld	r7		; the last digit dropped
	xch	r3
	ldm	0
	xch	r2
	bbl	0
5:	clb			; k >= 17: everything drops, the 17th is 0
7:	src	p1
	wrm
	isz	r3, 7b
6:	fim	p1, 0
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_b_clear(buf, -len): zero `len` characters from address `buf`.
	.section .text.__leaf_b_clear,"ax"
	.globl	__leaf_b_clear
__leaf_b_clear:
	clb
1:	src	p1
	wrm
	isz	r3, 2f
	inc	r2
2:	isz	r5, 1b
	isz	r4, 1b
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_b_up(buf, -len): shift a `len`-digit buffer up one place (x10), the top
; digit dropped and digit 0 = 0.
	.section .text.__leaf_b_up,"ax"
	.globl	__leaf_b_up
__leaf_b_up:
	ldm	0
	xch	r7		; carry digit
1:	src	p1
	rdm
	xch	r7
	wrm
	isz	r3, 2f
	inc	r2
2:	isz	r5, 1b
	isz	r4, 1b
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_b_addsh(at, o): add register o's 16 digits into the buffer at address
; `at` and ripple the carry upward (multiplication: the buffer holds a
; partial product below 10^32, so the carry is always absorbed inside it).
	.section .text.__leaf_b_addsh,"ax"
	.globl	__leaf_b_addsh
__leaf_b_addsh:
	ldm	0
	xch	r6		; 16 digits
	clc
1:	src	p2
	rdm
	src	p1
	adm
	daa
	wrm
	inc	r5
	isz	r3, 2f
	inc	r2
2:	isz	r6, 1b
3:	jcn	nc, 5f		; ripple
	src	p1
	ldm	0
	adm
	daa
	wrm
	isz	r3, 3b
	inc	r2
	jun	3b
5:	bbl	0

; ---------------------------------------------------------------------------
; __leaf_b_divdig(div) -> count: one long-division digit (`__n_div`). The
; remainder is the 17 digits at 0x9F..0xAF, below 10 * div; subtract div (its
; 16 digits, then a 0) while the remainder is not below it, counting. P0 walks
; the remainder, P1 the divisor (R3 = digit j), R4 = 0, R5 = count.
	.section .text.__leaf_b_divdig,"ax"
	.globl	__leaf_b_divdig
__leaf_b_divdig:
	fim	p2, 0
1:	fim	p0, 0x9F	; compare pass: CY = 1 means "no borrow yet"
	stc
2:	cmc			; SBM takes CY = 1 as "borrow in"
	src	p0
	rdm
	src	p1
	sbm
	isz	r1, 3f
	inc	r0
3:	isz	r3, 2b
	cmc			; the 17th digit against 0
	src	p0
	rdm
	sub	r4
	jcn	nc, 6f		; remainder < div: done
	fim	p0, 0x9F	; subtract pass (same walk)
	stc
4:	tcs
	src	p1
	sbm
	clc
	src	p0
	adm
	daa
	wrm
	isz	r1, 5f
	inc	r0
5:	isz	r3, 4b
	tcs
	clc
	src	p0
	adm
	daa
	wrm
	inc	r5
	jun	1b
6:	ld	r5
	xch	r3
	ldm	0
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_b_sqdig() -> x: one square-root digit (`__n_sqrt`). While the
; 20-digit remainder at 0x80 >= the 20-digit odd term at 0x94: rem -= s,
; s += 2, x += 1 (x <= 9 by the invariant rem <= 2*root). P0 / P3 walk rem /
; s, P2 = -20, R3 = x.
	.section .text.__leaf_b_sqdig,"ax"
	.globl	__leaf_b_sqdig
__leaf_b_sqdig:
	fim	p1, 0
1:	fim	p0, 0x80	; compare pass: rem - s, no write
	fim	p3, 0x94
	fim	p2, 0xEC	; -20
	stc
2:	cmc			; SBM takes CY = 1 as "borrow in"
	src	p0
	rdm
	src	p3
	sbm
	isz	r1, 3f
	inc	r0
3:	isz	r7, 4f
	inc	r6
4:	isz	r5, 2b
	isz	r4, 2b
	jcn	nc, 9f		; rem < s
	fim	p0, 0x80	; rem -= s (decimal)
	fim	p3, 0x94
	fim	p2, 0xEC
	stc
5:	tcs
	src	p3
	sbm
	clc
	src	p0
	adm
	daa
	wrm
	isz	r1, 6f
	inc	r0
6:	isz	r7, 7f
	inc	r6
7:	isz	r5, 5b
	isz	r4, 5b
	fim	p3, 0x94	; s += 2 (ripple)
	ldm	2
	clc
8:	src	p3
	adm
	daa
	wrm
	jcn	nc, 10f
	ldm	0
	isz	r7, 8b
	inc	r6
	jun	8b
10:	inc	r3
	jun	1b
9:	bbl	0

; ===========================================================================
; Entry routines: whole operations of `src/bcd.rs` (`Num::div`, ...). They
; call the leaves above (one level), clobber P0-P5 and keep state that must
; survive a call in free status characters of
; the scratch registers 0x80-0xB0 and 0xD0 (each routine lists its own).

; ---------------------------------------------------------------------------
; __n_div(dst|frac, a|fl, b) -> 1, or 0 on divide-by-zero or a quotient wider
; than 16 digits (dst unchanged). `Num::div`: R = floor(|a / b| * 10^fr), fr =
; frac, or in FL (fl = 1) 13 - (integer digits of the quotient), clamped to
; 0..13. Long division of A * 10^k by B (the digit strings, k = fr + sb - sa):
; per step the 17-digit remainder at 0x9F takes the next digit of A (then
; zeros), __leaf_b_divdig counts the quotient digit, and WIN (0xB0) shifts it in.
; Quotient digit j weighs 10^(base - 1 - j), base = n + sb - sa, so in FL the
; first nonzero one fixes fr and with it the step count base + fr.
; State: 0x80 S0 dst, S1 a, S2 b, S3 FL; 0x90 S1:S0 base, S3:S2 steps;
; 0xA0 S1:S0 j, S2 fr, S3 leading digit seen; 0xB0 S1:S0 n.
	.section .text.__n_div,"ax"
	.globl	__n_div
__n_div:
	fim	p0, 0x80
	src	p0
	ld	r2
	wr0
	ld	r4
	wr1
	ld	r6
	wr2
	ld	r5
	wr3
	fim	p0, 0xA0	; j = 0, fr = frac (13 in FL)
	src	p0
	ld	r3
	wr2
	ldm	0
	wr3
	wr0
	wr1
	ld	r5
	jcn	z, 1f
	ldm	13
	wr2
1:	ld	r6		; b == 0 fails
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	ld	r2
	jcn	nz, 2f
	ld	r3
	jcn	z, .Ldv_fail
2:	fim	p0, 0x80	; n -> 0xB0, R8:R9
	src	p0
	rd1
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	fim	p0, 0xB0
	src	p0
	ld	r3
	wr0
	ld	r2
	wr1
	fim	p0, 0x80	; + sb
	src	p0
	rd2
	xch	r0
	src	p0
	rd1
	clc
	add	r3
	xch	r9
	rd2
	add	r2
	xch	r8
	fim	p0, 0x80	; - sa
	src	p0
	rd1
	xch	r0
	src	p0
	rd1
	xch	r7
	rd2
	xch	r6
	clc
	ld	r9
	sub	r7
	xch	r9
	cmc
	ld	r8
	sub	r6
	xch	r8
	fim	p0, 0x90	; base = n + sb - sa
	src	p0
	ld	r9
	wr0
	ld	r8
	wr1
	jms	.Ldv_steps
	fim	p1, 0x9F	; remainder and WIN = 0
	fim	p2, 0xEF
	jms	__leaf_b_clear
	fim	p1, 0xB0
	fim	p2, 0xF0
	jms	__leaf_b_clear
.Ldv_loop:
	fim	p0, 0xA0	; while j < steps (signed)
	src	p0
	rd0
	xch	r9
	rd1
	xch	r8
	fim	p0, 0x90
	src	p0
	rd2
	clc
	sub	r9
	xch	r7
	cmc
	rd3
	sub	r8
	xch	r6
	ld	r6
	ral
	jcn	c, .Ldv_end
	ld	r6
	jcn	nz, 3f
	ld	r7
	jcn	z, .Ldv_end
3:	fim	p1, 0x9F	; remainder * 10 + the next digit
	fim	p2, 0xEF
	jms	__leaf_b_up
	fim	p0, 0xB0	; digit n - 1 - j of a, if j < n
	src	p0
	rd0
	xch	r7
	rd1
	xch	r6
	clc
	ld	r7
	sub	r9
	xch	r7
	cmc
	ld	r6
	sub	r8
	xch	r6
	jcn	nc, 4f
	ld	r7
	dac
	xch	r1
	jcn	c, 31f
	ld	r6
	jcn	z, 4f
31:	fim	p3, 0x80
	src	p3
	rd1
	xch	r0
	src	p0
	rdm
	jun	5f
4:	ldm	0
5:	fim	p0, 0x9F
	src	p0
	wrm
	fim	p0, 0x80	; count = divdig(b, 0x9F) -> R10
	src	p0
	rd2
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_b_divdig
	ld	r3
	xch	r10
	ld	r10		; FL: the leading digit fixes fr
	jcn	z, 6f
	fim	p0, 0x80
	src	p0
	rd3
	jcn	z, 6f
	fim	p0, 0xA0
	src	p0
	rd3
	jcn	nz, 6f
	ldm	1
	wr3
	rd0		; int = base - j
	xch	r9
	rd1
	xch	r8
	fim	p0, 0x90
	src	p0
	rd0
	clc
	sub	r9
	xch	r7
	cmc
	rd1
	sub	r8
	xch	r6
	ldm	13		; fr = 13 - clamp(int, 0, 13)
	xch	r11
	ld	r6
	ral
	jcn	c, 7f
	ld	r6
	jcn	nz, 61f
	clc
	ldm	13
	sub	r7
	xch	r11
	jcn	c, 7f
61:	ldm	0
	xch	r11
7:	fim	p0, 0xA0
	src	p0
	ld	r11
	wr2
	jms	.Ldv_steps
6:	fim	p0, 0xBF	; a 17th quotient digit fails
	src	p0
	rdm
	jcn	nz, .Ldv_fail
	fim	p1, 0xB0
	fim	p2, 0xF0
	jms	__leaf_b_up
	fim	p0, 0xB0
	src	p0
	ld	r10
	wrm
	fim	p0, 0xA0	; j += 1
	src	p0
	rd0
	iac
	wr0
	jcn	nc, .Ldv_loop
	rd1
	iac
	wr1
	jun	.Ldv_loop
.Ldv_end:
	fim	p0, 0x80	; dst = WIN, sign a ^ b, scale fr; zero: + and 0
	src	p0
	rd0
	xch	r2
	ldm	0
	xch	r3
	fim	p2, 0xB0
	jms	__leaf_n_cpyd
	fim	p0, 0x80
	src	p0
	rd0
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	fim	p0, 0x80
	src	p0
	rd1
	xch	r0
	src	p0
	rd0
	xch	r7
	fim	p0, 0x80
	src	p0
	rd2
	xch	r0
	src	p0
	rd0
	clc
	add	r7
	rar
	tcc
	xch	r7
	fim	p0, 0xA0
	src	p0
	rd2
	xch	r6
	ld	r2
	jcn	nz, 8f
	ld	r3
	jcn	nz, 8f
	ldm	0
	xch	r7
	ldm	0
	xch	r6
8:	fim	p0, 0x80
	src	p0
	rd0
	xch	r0
	src	p0
	ld	r7
	wr0
	ld	r6
	wr1
	ldm	0
	wr2
	fim	p1, 1
	bbl	0
.Ldv_fail:
	fim	p1, 0
	bbl	0

.Ldv_steps:			; steps = base + fr (0x90 S3:S2)
	fim	p0, 0xA0
	src	p0
	rd2
	xch	r7
	ldm	0
	xch	r6
	fim	p0, 0x90
	src	p0
	rd0
	clc
	add	r7
	wr2
	rd1
	add	r6
	wr3
	bbl	0

; ---------------------------------------------------------------------------
; __leaf_n_width(a) -> digits printed: max(digits, scale + 1) (scale a byte).
	.section .text.__leaf_n_width,"ax"
	.globl	__leaf_n_width
__leaf_n_width:
	ldm	15
	xch	r3
1:	src	p1
	rdm
	jcn	nz, 2f
	ld	r3
	jcn	z, 3f
	dac
	xch	r3
	jun	1b
2:	ld	r3		; digits = index + 1 -> R4:R5
	iac
	xch	r5
	tcc
	xch	r4
	jun	4f
3:	fim	p2, 0
4:	src	p1		; scale -> R6:R7
	rd1
	xch	r7
	rd2
	xch	r6
	clc		; scale >= digits ?
	ld	r7
	sub	r5
	cmc
	ld	r6
	sub	r4
	jcn	c, 5f
	ld	r4
	xch	r2
	ld	r5
	xch	r3
	bbl	0
5:	ld	r7		; scale + 1
	iac
	xch	r3
	tcc
	add	r6
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; __n_mul(dst, a, b) -> 1, or 0 if the product needs more than 16 digits
; (dst unchanged). `Num::mul`: b is added, shifted by i, a[i] times into the
; 32-digit buffer at 0x80. State: R8 dst, R9 a, R10 b, R11 i; the repeat
; count in status 0xA0 S0; then R11 sign, R10:R9 scale (dst may alias an
; operand: __leaf_n_cpyd writes digits only, so both are read first).
	.section .text.__n_mul,"ax"
	.globl	__n_mul
__n_mul:
	ld	r2
	xch	r8
	ld	r4
	xch	r9
	ld	r6
	xch	r10
	fim	p1, 0x80
	fim	p2, 0xE0	; -32
	jms	__leaf_b_clear
	ldm	0
	xch	r11
1:	ld	r9		; k = a[i]
	xch	r0
	ld	r11
	xch	r1
	src	p0
	rdm
	fim	p0, 0xA0
	src	p0
	wr0
2:	fim	p0, 0xA0
	src	p0
	rd0
	jcn	z, 3f
	dac
	wr0
	ldm	8		; buffer at 0x80 + i += b
	xch	r2
	ld	r11
	xch	r3
	ld	r10
	xch	r4
	ldm	0
	xch	r5
	jms	__leaf_b_addsh
	jun	2b
3:	isz	r11, 1b
	fim	p1, 0x90	; digits 16..31 must be zero
	jms	__leaf_n_digits
	ld	r2
	jcn	nz, .Lml_fail
	ld	r3
	jcn	nz, .Lml_fail
	ld	r9		; sign a ^ b, scale sa + sb (read before dst, which
	xch	r0		; may alias an operand)
	ldm	0
	xch	r1
	src	p0
	rd0
	xch	r6
	rd1
	xch	r5
	rd2
	xch	r4
	ld	r10
	xch	r0
	src	p0
	rd0
	clc
	add	r6
	rar
	tcc
	xch	r6
	rd1
	clc
	add	r5
	xch	r5
	rd2
	add	r4
	xch	r4
	ld	r6		; R11 = sign, R10:R9 = scale (leaves keep P4:P5)
	xch	r11
	ld	r5
	xch	r9
	ld	r4
	xch	r10
	ld	r8		; dst digits = the low 16 (status untouched)
	xch	r2
	ldm	0
	xch	r3
	fim	p2, 0x80
	jms	__leaf_n_cpyd
	ld	r8
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	ld	r8
	xch	r0
	ldm	0
	xch	r1
	ld	r2
	jcn	nz, 4f
	ld	r3
	jcn	nz, 4f
	src	p0		; zero: sign +, scale 0
	ldm	0
	wr0
	wr1
	wr2
	jun	5f
4:	src	p0
	ld	r11
	wr0
	ld	r9
	wr1
	ld	r10
	wr2
5:	fim	p1, 1
	bbl	0
.Lml_fail:
	fim	p1, 0
	bbl	0

; ---------------------------------------------------------------------------
; __n_round(dst, a) -> 1 if it rounded up. `Num::round_dp`: dst = a to `frac`
; = the DP selector (0xC2 S2) fractional digits, truncated, or rounded half
; up when the rounding switch (0xC3 S3) is 5/4 (1); a zero result gets sign
; +. State: R8 dst, R9 frac, R10 switch - 1 (0: round; then "rounded"), R11 a.
	.section .text.__n_round,"ax"
	.globl	__n_round
__n_round:
	ld	r2
	xch	r8
	ld	r4
	xch	r11
	fim	p0, 0xC0
	src	p0
	rd2
	xch	r9
	rd3
	dac
	xch	r10
	clc
	ld	r8
	sub	r11
	jcn	z, 1f
	ld	r8		; dst = a (packed: R2 dst, R3 src)
	xch	r2
	ld	r11
	xch	r3
	jms	__leaf_n_cp
1:	ld	r8		; s = scale
	xch	r0
	ldm	0
	xch	r1
	src	p0
	rd1
	xch	r7
	rd2
	xch	r6
	clc		; R4:R5 = frac - s ; CY = 1 iff frac >= s
	ld	r9
	sub	r7
	xch	r5
	cmc
	ldm	0
	sub	r6
	xch	r4
	ld	r8
	xch	r2
	ldm	0
	xch	r3
	jcn	nc, 2f
	jms	__leaf_n_shl	; frac >= s: pad with zeros (unchanged if it overflows)
	ld	r3
	jcn	z, 9f
	ld	r8
	xch	r0
	ldm	0
	xch	r1
	src	p0
	ld	r9
	wr1
	ldm	0
	wr2
9:	fim	p1, 0
	bbl	0
2:	clb		; frac < s: drop s - frac digits
	sub	r5
	xch	r5
	cmc
	ldm	0
	sub	r4
	xch	r4
	jms	__leaf_n_shr	; R3 = most significant dropped digit
	ld	r10
	jcn	nz, 3f
	ldm	5
	xch	r7
	clc
	ld	r3
	sub	r7
	jcn	c, 4f		; >= 5
3:	ldm	0
	xch	r10
	jun	5f
4:	ldm	1
	xch	r10
5:	ld	r8		; scale = frac
	xch	r0
	ldm	0
	xch	r1
	src	p0
	ld	r9
	wr1
	ldm	0
	wr2
	ld	r10
	jcn	z, 6f
	ld	r8		; +1 ulp; a carry out of digit 15 is not "rounded"
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_inc
	ld	r3
	jcn	z, 6f
	ldm	0
	xch	r10
6:	ld	r8		; zero: sign +
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	ld	r2
	jcn	nz, 7f
	ld	r3
	jcn	nz, 7f
	ld	r8
	xch	r0
	ldm	0
	xch	r1
	src	p0
	wr0
7:	ld	r10
	xch	r3
	ldm	0
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; __n_add(dst|negate, a, b) -> 1, or 0 when the result does not fit 14 digits
; (dst unchanged). `Num::add_signed`: XA = a, XB = b (sign flipped when
; negate), both rescaled to the larger scale, then the magnitude sum or
; difference; dst may alias a or b. State: R8 dst, R9 negate, R10:R11 scale.
	.section .text.__n_add,"ax"
	.globl	__n_add
__n_add:
	ld	r2
	xch	r8
	ld	r3
	xch	r9
	ld	r6
	xch	r10
	fim	p1, 0x80	; XA = a
	jms	__leaf_n_copy
	ldm	9		; XB = b
	xch	r2
	ld	r10
	xch	r3
	jms	__leaf_n_cp
	ld	r9
	jcn	z, 1f
	fim	p0, 0x90	; XB sign = 1 - sign
	src	p0
	rd0
	xch	r7
	ldm	1
	clc
	sub	r7
	wr0
1:	fim	p0, 0x80	; s = max(sa, sb) -> R10:R11
	src	p0
	rd1
	xch	r11
	rd2
	xch	r10
	fim	p0, 0x90
	src	p0
	rd1
	xch	r7
	rd2
	xch	r6
	clc
	ld	r11
	sub	r7
	cmc
	ld	r10
	sub	r6
	jcn	c, 2f
	ld	r6
	xch	r10
	ld	r7
	xch	r11
2:	ldm	8		; XA, XB to scale s: s >= their scale, so x 10^(s - cur)
	xch	r9		; (negate is spent)
3:	ld	r9
	xch	r0
	ldm	0
	xch	r1
	src	p0
	rd1
	xch	r7
	rd2
	xch	r6
	clc
	ld	r11
	sub	r7
	xch	r5
	cmc
	ld	r10
	sub	r6
	xch	r4
	ld	r9
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_shl
	ld	r3
	jcn	z, .Lad_fail
	ld	r9
	xch	r0
	ldm	0
	xch	r1
	src	p0
	ld	r11
	wr1
	ld	r10
	wr2
	inc	r9
	clc
	ldm	10
	sub	r9
	jcn	nz, 3b
	fim	p0, 0x80	; same sign: add magnitudes
	src	p0
	rd0
	xch	r6
	fim	p0, 0x90
	src	p0
	rd0
	clc
	sub	r6
	jcn	nz, 3f
	fim	p1, 0x80
	fim	p2, 0x90
	jms	__leaf_n_add16
	ld	r3
	jcn	nz, .Lad_fail
	ldm	8
	jun	5f
3:	fim	p1, 0x80	; else subtract the smaller magnitude
	fim	p2, 0x90
	jms	__leaf_n_cmp
	ld	r3
	jcn	z, 4f
	fim	p1, 0x80
	fim	p2, 0x90
	jms	__leaf_n_sub16
	ldm	8
	jun	5f
4:	fim	p1, 0x90
	fim	p2, 0x80
	jms	__leaf_n_sub16
	ldm	9
5:	xch	r10		; R10 = the result's register (8 or 9)
	ld	r10
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	ld	r2
	jcn	nz, 6f
	ld	r3
	jcn	nz, 6f
	ld	r10		; zero: sign +
	xch	r0
	ldm	0
	xch	r1
	src	p0
	wr0
6:	ld	r10		; width > 14 fails
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_width
	ld	r2
	jcn	nz, .Lad_fail
	ld	r3
	iac
	jcn	c, .Lad_fail
	ld	r8		; dst = result
	xch	r2
	ld	r10
	xch	r3
	jms	__leaf_n_cp
	fim	p1, 1
	bbl	0
.Lad_fail:
	fim	p1, 0
	bbl	0

; ---------------------------------------------------------------------------
; __n_sqrt(dst, a, frac): dst = sqrt(|a|) to `frac` (< 16) digits, truncated.
; `Num::sqrt_at`: digit pairs of the radicand (digit k = a[k - off], off =
; 2*frac - a.scale) brought down from the top; each root digit counts the odd
; terms subtracted (__leaf_b_sqdig). Remainder 0x80 (20 digits), odd term 0x94
; (20), root WIN 0xB0. State: 0xA0 S0 dst, S1 a, S2 frac; 0xD0 S1:S0 off,
; S3:S2 pairs left.
	.section .text.__n_sqrt,"ax"
	.globl	__n_sqrt
__n_sqrt:
	fim	p0, 0xA0
	src	p0
	ld	r2
	wr0
	ld	r4
	wr1
	ld	r7
	wr2
	ld	r4		; sa -> R8:R9
	xch	r0
	ldm	0
	xch	r1
	src	p0
	rd1
	xch	r9
	rd2
	xch	r8
	clc		; off = 2*frac - sa -> R10:R11
	ld	r7
	add	r7
	xch	r11
	tcc
	xch	r10
	clc
	ld	r11
	sub	r9
	xch	r11
	cmc
	ld	r10
	sub	r8
	xch	r10
	fim	p0, 0xD0
	src	p0
	ld	r11
	wr0
	ld	r10
	wr1
	fim	p4, 0		; hi = top valid radicand index with a digit
	ld	r10
	xch	r6
	ld	r11
	xch	r7		; idx = i + off
	fim	p0, 0xA0
	src	p0
	rd1
	xch	r4
	ldm	0
	xch	r5		; P2 = a, R5 = i
1:	src	p2
	rdm
	jcn	z, 2f
	ld	r6		; idx < 0
	ral
	jcn	c, 2f
	clc		; idx >= 34
	ldm	1
	sub	r7
	cmc
	ldm	2
	sub	r6
	jcn	nc, 2f
	ld	r6
	xch	r8
	ld	r7
	xch	r9
2:	ld	r7
	iac
	xch	r7
	tcc
	add	r6
	xch	r6
	isz	r5, 1b
	clc		; pairs = hi / 2 + 1
	ld	r8
	rar
	xch	r8
	ld	r9
	rar
	xch	r9
	ld	r9
	iac
	xch	r9
	tcc
	add	r8
	xch	r8
	fim	p0, 0xD0
	src	p0
	ld	r9
	wr2
	ld	r8
	wr3
	fim	p1, 0x80	; remainder and odd term = 0, odd term = 1
	fim	p2, 0xD8	; -40
	jms	__leaf_b_clear
	fim	p0, 0x94
	src	p0
	ldm	1
	wrm
	fim	p1, 0xB0	; root = 0
	fim	p2, 0xF0
	jms	__leaf_b_clear
3:	fim	p0, 0xD0	; while pairs > 0
	src	p0
	rd2
	xch	r9
	rd3
	xch	r8
	ld	r8
	jcn	nz, 4f
	ld	r9
	jcn	z, .Lsq_done
4:	ld	r9		; pairs -= 1 (P4 = p)
	dac
	xch	r9
	jcn	c, 5f
	ld	r8
	dac
	xch	r8
5:	src	p0
	ld	r9
	wr2
	ld	r8
	wr3
	fim	p1, 0x80	; remainder *= 100
	fim	p2, 0xEC
	jms	__leaf_b_up
	fim	p1, 0x80
	fim	p2, 0xEC
	jms	__leaf_b_up
	clc		; i0 = 2p - off
	ld	r9
	add	r9
	xch	r7
	ld	r8
	add	r8
	xch	r6
	fim	p0, 0xD0
	src	p0
	rd0
	xch	r5
	rd1
	xch	r4
	clc
	ld	r7
	sub	r5
	xch	r7
	cmc
	ld	r6
	sub	r4
	xch	r6
	jms	.Lsq_pair	; the next digit pair of the radicand
	jms	__leaf_b_sqdig	; x = the next root digit
	ld	r3
	xch	r10
	fim	p1, 0xB0	; root = root * 10 + x
	fim	p2, 0xF0
	jms	__leaf_b_up
	fim	p0, 0xB0
	src	p0
	ld	r10
	wrm
	fim	p0, 0x94	; odd term = (odd term - 1) * 10 + 1
	src	p0
	rdm
	dac
	wrm
	fim	p1, 0x94
	fim	p2, 0xEC
	jms	__leaf_b_up
	fim	p0, 0x94
	src	p0
	ldm	1
	wrm
	jun	3b
.Lsq_pair:		; RAM 0x80, 0x81 = digits i, i + 1 (R6:R7, signed) of a, or 0
	ldm	0
	xch	r5
1:	ld	r6
	jcn	nz, 2f
	fim	p0, 0xA0
	src	p0
	rd1
	xch	r0
	ld	r7
	xch	r1
	src	p0
	rdm
	jun	3f
2:	ldm	0
3:	xch	r4
	ldm	8
	xch	r0
	ld	r5
	xch	r1
	src	p0
	ld	r4
	wrm
	ld	r7
	iac
	xch	r7
	tcc
	add	r6
	xch	r6
	inc	r5
	ld	r5
	dac
	jcn	z, 1b
	bbl	0

.Lsq_done:
	fim	p0, 0xA0	; dst = root, sign +, scale frac (0 for zero)
	src	p0
	rd0
	xch	r8		; R8 = dst, R9 = frac (leaves keep P4)
	rd2
	xch	r9
	ld	r8
	xch	r2
	ldm	0
	xch	r3
	fim	p2, 0xB0
	jms	__leaf_n_cpyd
	ld	r8
	xch	r2
	ldm	0
	xch	r3
	jms	__leaf_n_digits
	ld	r8
	xch	r0
	ldm	0
	xch	r1
	ld	r2
	jcn	nz, 6f
	ld	r3
	jcn	nz, 6f
	ldm	0
	xch	r9
6:	src	p0
	ldm	0
	wr0
	ld	r9
	wr1
	ldm	0
	wr2
	bbl	0

; ---------------------------------------------------------------------------
; __n_compute() -> 1, or 0 for no pending operation or an overflow:
; `Calculator::compute` (lib.rs). T2 = DR (op) WR for MOP (flag 0x43 S3:
; 1 mul, 2 div, 3 const div, 4 const mul); the constant ones first move DR =
; WR, WR = CR. Ends in a tail jump to __n_mul / __n_div (FL quotient), so it
; adds no stack level to theirs. R8 = op.
	.section .text.__n_compute,"ax"
	.globl	__n_compute
__n_compute:
	fim	p0, 0x40
	src	p0
	rd3
	xch	r8
	ld	r8
	jcn	z, 3f		; none
	clc			; op >= 3: DR = WR, WR = CR
	ldm	2
	sub	r8
	jcn	c, 1f
	fim	p1, 0x10
	jms	__leaf_n_cp
	fim	p1, 0x02
	jms	__leaf_n_cp
1:	fim	p1, 0x70	; T2 = DR (op) WR
	fim	p2, 0x10
	fim	p3, 0x00
	ld	r8		; mul: 1, 4
	dac
	jcn	z, 2f
	ldm	4
	clc
	sub	r8
	jcn	z, 2f
	fim	p2, 0x11	; div: DR | FL
	jun	__n_div
2:	jun	__n_mul
3:	fim	p1, 0
	bbl	0
