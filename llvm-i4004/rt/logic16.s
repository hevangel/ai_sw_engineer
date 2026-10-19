; libi4004rt -- 16-bit bitwise logic helpers (__andhi3 / __orhi3 / __xorhi3)
; ============================================================================
; Contract (abi.md sec.4.1): i16 arguments arrive in the double-pairs
;   a = D1 = P2:P3   (P2 = a low byte  = R4:R5,  P3 = a high byte = R6:R7)
;   b = D2 = P4:P5   (P4 = b low byte  = R8:R9,  P5 = b high byte = R10:R11)
; Result i16 is returned in D1 = P2:P3 (low byte P2, high byte P3).
;
; Bitwise ops are per-bit independent, so each output byte is just the qi3
; boolean of the matching input bytes. We reuse the EXACT bit-serial method of
; logic.s (__{and,or,xor}qi3): for each of the 8 bits, extract a_bit and b_bit,
; form s = a_bit + b_bit in {0,1,2}, and derive the boolean with no logic op:
;   and = s bit1 (s==2) ; or = (s != 0) ; xor = s bit0.
; Done once for the low byte (a=P2,b=P4 -> P2) and once for the high byte
; (a=P3,b=P5 -> P3). Scratch: P0 (R0:R1) holds {bit, counter}; ACC/CY.
; P1,P4,P5 are free scratch across the second byte once the first is stored.
; Leaf helpers (no JMS, no stack); touch only the caller-clobbered set
; ACC,CY,P0,P1 plus the arg/result pairs P2..P5. Execution-tested on chips
; (rt/tests/run_rt_tests.sh, --wide).
;
; R0 = loop counter, R1 = extracted bit / boolean scratch.

; ---------------------------------------------------------------------------
; __andhi3 : (a:D1) & (b:D2) -> D1
; ---------------------------------------------------------------------------
	.section .text.__andhi3,"ax"
	.globl	__andhi3
__andhi3:
	; --- low byte: a=P2 (r4:r5), b=P4 (r8:r9) -> result accumulates in P2 ---
	; build result in P1 (r2:r3) to avoid disturbing a until the byte is done
	ldm	0
	xch	r2
	ldm	0
	xch	r3			; P1 (result) = 0
	ldm	8
	xch	r0			; 8 bits
.Land16_lo:
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4			; a.lo <<= 1 ; CY = a_bit (MSB first)
	tcc
	xch	r1			; R1 = a_bit
	clc
	ld	r9
	ral
	xch	r9
	ld	r8
	ral
	xch	r8			; b.lo <<= 1 ; CY = b_bit
	tcc
	add	r1			; ACC = s (0..2)
	rar				; bit0 <- old bit1 = AND
	xch	r1			; R1 = and bit (bit0); high bits zero (s<=2)
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2			; result <<= 1
	ld	r1
	rar				; CY <- and bit
	jcn	nc, .Land16_lo_n
	ld	r3
	iac
	xch	r3			; set result bit0
.Land16_lo_n:
	isz	r0, .Land16_lo
	ld	r2
	xch	r4
	ld	r3
	xch	r5			; a.lo-slot P2 := result low byte
	; --- high byte: a=P3 (r6:r7), b=P5 (r10:r11) -> P3 ---
	ldm	0
	xch	r2
	ldm	0
	xch	r3			; P1 (result) = 0
	ldm	8
	xch	r0
.Land16_hi:
	clc
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6			; a.hi <<= 1 ; CY = a_bit
	tcc
	xch	r1
	clc
	ld	r11
	ral
	xch	r11
	ld	r10
	ral
	xch	r10			; b.hi <<= 1 ; CY = b_bit
	tcc
	add	r1
	rar
	xch	r1			; R1 = and bit
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2			; result <<= 1
	ld	r1
	rar
	jcn	nc, .Land16_hi_n
	ld	r3
	iac
	xch	r3
.Land16_hi_n:
	isz	r0, .Land16_hi
	ld	r2
	xch	r6
	ld	r3
	xch	r7			; P3 := result high byte
	bbl	0

; ---------------------------------------------------------------------------
; __orhi3 : (a:D1) | (b:D2) -> D1  (or = 1 iff s != 0)
; ---------------------------------------------------------------------------
	.section .text.__orhi3,"ax"
	.globl	__orhi3
__orhi3:
	ldm	0
	xch	r2
	ldm	0
	xch	r3
	ldm	8
	xch	r0
.Lor16_lo:
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4
	tcc
	xch	r1
	clc
	ld	r9
	ral
	xch	r9
	ld	r8
	ral
	xch	r8
	tcc
	add	r1			; ACC = s
	xch	r1			; R1 = s
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2			; result <<= 1
	ld	r1
	jcn	z, .Lor16_lo_n		; s == 0 -> or bit 0
	ld	r3
	iac
	xch	r3
.Lor16_lo_n:
	isz	r0, .Lor16_lo
	ld	r2
	xch	r4
	ld	r3
	xch	r5
	ldm	0
	xch	r2
	ldm	0
	xch	r3
	ldm	8
	xch	r0
.Lor16_hi:
	clc
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6
	tcc
	xch	r1
	clc
	ld	r11
	ral
	xch	r11
	ld	r10
	ral
	xch	r10
	tcc
	add	r1
	xch	r1			; R1 = s
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2
	ld	r1
	jcn	z, .Lor16_hi_n
	ld	r3
	iac
	xch	r3
.Lor16_hi_n:
	isz	r0, .Lor16_hi
	ld	r2
	xch	r6
	ld	r3
	xch	r7
	bbl	0

; ---------------------------------------------------------------------------
; __xorhi3 : (a:D1) ^ (b:D2) -> D1  (xor = s bit0)
; ---------------------------------------------------------------------------
	.section .text.__xorhi3,"ax"
	.globl	__xorhi3
__xorhi3:
	ldm	0
	xch	r2
	ldm	0
	xch	r3
	ldm	8
	xch	r0
.Lxor16_lo:
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4
	tcc
	xch	r1
	clc
	ld	r9
	ral
	xch	r9
	ld	r8
	ral
	xch	r8
	tcc
	add	r1			; ACC = s
	rar				; CY <- s bit0 = xor
	tcc				; ACC = xor bit
	xch	r1
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2
	ld	r1
	jcn	z, .Lxor16_lo_n
	ld	r3
	iac
	xch	r3
.Lxor16_lo_n:
	isz	r0, .Lxor16_lo
	ld	r2
	xch	r4
	ld	r3
	xch	r5
	ldm	0
	xch	r2
	ldm	0
	xch	r3
	ldm	8
	xch	r0
.Lxor16_hi:
	clc
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6
	tcc
	xch	r1
	clc
	ld	r11
	ral
	xch	r11
	ld	r10
	ral
	xch	r10
	tcc
	add	r1
	rar
	tcc
	xch	r1
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2
	ld	r1
	jcn	z, .Lxor16_hi_n
	ld	r3
	iac
	xch	r3
.Lxor16_hi_n:
	isz	r0, .Lxor16_hi
	ld	r2
	xch	r6
	ld	r3
	xch	r7
	bbl	0
