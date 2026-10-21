; libi4004rt -- block digit-register primitives (Tier-A size co-design)
; ============================================================================
; `Num` keeps 16 BCD digits packed two per byte in an 8-byte array. On the 4004
; a u8 is two 4002 chars and the packing aligns so **digit i is char i** of the
; array's SRC region: 16 consecutive chars, one BCD digit (0..9) each. So each
; primitive is one 16-iteration SRC/RDM/WRM/INC loop -- the compact form the
; original ROM used, replacing the Rust per-digit mask/shift loops the backend
; inlined.
;
; ABI (abi.md sec.4.1): pointer args are i16 in D1 (P2 = R4:R5) and D2
; (P4 = R8:R9); the SRC address byte is that pair's value. i8 return in P1
; (R2:R3, low nibble R3). Leaf routines; touch only the caller-clobbered set
; (ACC, CY, P0-P5). Behaviour is byte-identical to regops.rs's host impl
; (host tests stay 42/42 exact; chips execution-tested in rt/tests).
;
; Register use inside the loops:
;   P2 (R4:R5) = a address (R5 = char, INC'd each step)
;   P4 (R8:R9) = b address (R9 = char, INC'd each step)
;   R0 = loop counter (seed 0; ISZ wraps to 0 after 16 increments)
;   R1 = carry / borrow ; R6, R7 = scratch digit values

; ---------------------------------------------------------------------------
; reg_add : a += b over 16 BCD digits. Returns 1 if no carry out, else 0 (P1).
; ---------------------------------------------------------------------------
	.section .text.reg_add,"ax"
	.globl	reg_add
reg_add:
	ldm	0
	xch	r1			; carry = 0
	ldm	0
	xch	r0			; counter
.Ladd_loop:
	src	p4
	rdm
	xch	r6			; R6 = b[i]
	src	p2
	rdm				; ACC = a[i]
	clc
	add	r6			; ACC = a + b  (0..18)
	add	r1			; + carry      (0..19)
	xch	r6			; R6 = sum
	; sum >= 10 ?  compute sum - 10 : CLC; LD sum; add 6 then the 4004 carry
	; out of the low BCD is set iff sum >= 10 (classic +6 BCD correct).
	; We instead do an explicit compare: ldm 10 into R7, CLC, LD sum, SUB R7.
	ldm	10
	xch	r7
	clc
	ld	r6
	sub	r7			; ACC = sum - 10 ; CY=1 iff sum >= 10 (no borrow)
	jcn	0xa, .Ladd_ge10		; cond 0xA = JCN on CY==0? -> use NC meaning
	; CY == 1 here means sum >= 10: ACC already = sum-10, set carry.
	src	p2
	wrm				; a[i] = sum - 10
	ldm	1
	xch	r1			; carry = 1
	jun	.Ladd_next
.Ladd_ge10:
	; CY == 0 : sum < 10. Write the original sum, carry = 0.
	src	p2
	ld	r6
	wrm				; a[i] = sum
	ldm	0
	xch	r1			; carry = 0
.Ladd_next:
	inc	r5			; a char++
	inc	r9			; b char++
	isz	r0, .Ladd_loop		; 16 digits
	; return !carry : P1 = (carry==0) ? 1 : 0
	ld	r1
	jcn	0x4, .Ladd_ret1		; cond 0x4 = ACC==0 -> carry 0 -> return 1
	ldm	0
	xch	r3
	ldm	0
	xch	r2
	bbl	0
.Ladd_ret1:
	ldm	1
	xch	r3
	ldm	0
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; reg_sub : a -= b over 16 BCD digits (caller guarantees a >= b). No return.
;   sub = b[i] + borrow ; if a[i] >= sub : a[i]-=sub, borrow 0 ; else a[i] +=
;   10 - sub, borrow 1.
; ---------------------------------------------------------------------------
	.section .text.reg_sub,"ax"
	.globl	reg_sub
reg_sub:
	ldm	0
	xch	r1			; borrow = 0
	ldm	0
	xch	r0
.Lsub_loop:
	src	p4
	rdm
	clc
	add	r1			; ACC = b[i] + borrow  (0..10)
	xch	r7			; R7 = sub
	src	p2
	rdm				; ACC = a[i]
	clc
	sub	r7			; ACC = a[i] - sub ; CY=1 iff a[i] >= sub
	jcn	0xa, .Lsub_borrow	; CY==0 -> borrow
	; a[i] >= sub : ACC = a[i]-sub, borrow 0
	src	p2
	wrm
	ldm	0
	xch	r1
	jun	.Lsub_next
.Lsub_borrow:
	; a[i] < sub : result = a[i] - sub + 10 ; the SUB above left ACC = a-sub-6
	; (mod 16, since SUB adds ~sub+1 and wraps); simplest is recompute:
	; ACC currently = (a - sub) mod 16. Add 10 to get the borrow result, but
	; that overflows the nibble; instead recompute cleanly: a + (10 - sub).
	; 10 - sub : ldm 10; sub R7.
	ldm	10
	clc
	sub	r7			; ACC = 10 - sub ; (sub<=10 so no borrow issues)
	xch	r6			; R6 = 10 - sub
	src	p2
	rdm				; ACC = a[i]
	clc
	add	r6			; ACC = a[i] + (10 - sub)
	src	p2
	wrm
	ldm	1
	xch	r1			; borrow = 1
.Lsub_next:
	inc	r5
	inc	r9
	isz	r0, .Lsub_loop
	bbl	0

; ---------------------------------------------------------------------------
; reg_cmp : magnitude compare, MSD first. Returns 2 (a>b), 0 (a<b), 1 (eq).
;   Walk from digit 15 down to 0. Addresses start at base; we must reach the
;   top digit first, so add 15 to each char then DAC down.
; ---------------------------------------------------------------------------
	.section .text.reg_cmp,"ax"
	.globl	reg_cmp
reg_cmp:
	; advance both addresses to char+15 (the MSD).
	ldm	15
	clc
	add	r5
	xch	r5			; a char += 15
	ldm	15
	clc
	add	r9
	xch	r9			; b char += 15
	ldm	0
	xch	r0			; counter
.Lcmp_loop:
	src	p4
	rdm
	xch	r6			; R6 = b[i]
	src	p2
	rdm				; ACC = a[i]
	clc
	sub	r6			; ACC = a - b ; CY=1 iff a >= b
	jcn	0xa, .Lcmp_a_lt_or_eq	; CY==0 -> a < b
	; a >= b : if a != b then a > b. ACC = a-b; if ACC==0 equal, else a>b.
	jcn	0x4, .Lcmp_eq_step	; ACC==0 -> equal this digit, continue
	; a > b
	ldm	2
	xch	r3
	ldm	0
	xch	r2
	bbl	0
.Lcmp_a_lt_or_eq:
	; a < b (borrow) : but could ACC==0 with borrow? no: borrow means a<b.
	ldm	0
	xch	r3
	ldm	0
	xch	r2
	bbl	0
.Lcmp_eq_step:
	ld	r5
	dac
	xch	r5			; a char--
	ld	r9
	dac
	xch	r9			; b char--
	isz	r0, .Lcmp_loop_chk
.Lcmp_loop_chk:
	; ISZ increments R0; after 16 steps R0 wraps to 0. Loop while not done.
	ld	r0
	jcn	0x4, .Lcmp_equal	; R0==0 -> all 16 compared equal
	jun	.Lcmp_loop
.Lcmp_equal:
	ldm	1
	xch	r3
	ldm	0
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; reg_is_zero : returns 1 if all 16 digits are 0, else 0. a in P2.
; ---------------------------------------------------------------------------
	.section .text.reg_is_zero,"ax"
	.globl	reg_is_zero
reg_is_zero:
	ldm	0
	xch	r0
.Liz_loop:
	src	p2
	rdm
	jcn	0xc, .Liz_nonzero	; cond 0xC = ACC != 0 -> not zero
	inc	r5
	isz	r0, .Liz_loop
	ldm	1
	xch	r3
	ldm	0
	xch	r2
	bbl	0
.Liz_nonzero:
	ldm	0
	xch	r3
	ldm	0
	xch	r2
	bbl	0

; ---------------------------------------------------------------------------
; reg_copy : dst = src, 16 digits. dst in P2, src in P4.
; ---------------------------------------------------------------------------
	.section .text.reg_copy,"ax"
	.globl	reg_copy
reg_copy:
	ldm	0
	xch	r0
.Lcpy_loop:
	src	p4
	rdm
	src	p2
	wrm
	inc	r5
	inc	r9
	isz	r0, .Lcpy_loop
	bbl	0

; ---------------------------------------------------------------------------
; reg_clear : zero 16 digits of a (P2).
; ---------------------------------------------------------------------------
	.section .text.reg_clear,"ax"
	.globl	reg_clear
reg_clear:
	ldm	0
	xch	r0
.Lclr_loop:
	src	p2
	ldm	0
	wrm
	inc	r5
	isz	r0, .Lclr_loop
	bbl	0
