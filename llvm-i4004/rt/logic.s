; libi4004rt -- bitwise logic helpers (the 4004 has no AND/OR/XOR)
; ============================================================================
; __andqi3 / __orqi3 / __xorqi3 : (i8 a=P1) OP (i8 b=P2) -> i8 (P1).
;
; The MCS-4 ALU has only ADD/SUB and the rotates RAL/RAR -- no bitwise logic.
; We synthesize it BIT-SERIALLY over the 8 bits, MSB first. No ROM table (so
; nothing page-local to place, unlike the FIN nibble-table option in abi.md
; sec.6) and no RAM scratch.
;
; Per bit we extract a_bit and b_bit into the accumulator and form the 2-bit
; sum  s = a_bit + b_bit  in {0,1,2}, from which every boolean falls out with
; no logic op:
;     xor = s bit0            (1 iff exactly one input bit set)
;     and = s bit1            (1 iff both set, s==2)
;     or  = a_bit + b_bit clamped to 1 = (s bit0) | (s bit1); since s<=2 the two
;           bits are never both 1, so  or = (s bit0) + (s bit1)  with no carry,
;           equivalently or = 1 iff s != 0.
; The result byte is built by rotating it left (new bit enters at bit0) each
; step, then conditionally setting bit0 with IAC when the boolean is 1.
;
; Three separate functions so the op never needs a register: live values are
; a(R2:R3), b(R4:R5), result(R6:R7), counter(R0); R1 is the only scratch.
; Leaf helpers, no stack. See README for sizes.

; --- shared macro-ish prologue is inlined per function for page-locality ----

; ---------------------------------------------------------------------------
; __andqi3
; ---------------------------------------------------------------------------
	.section .text.__andqi3,"ax"
	.globl	__andqi3
__andqi3:
	ldm	0
	xch	r6
	ldm	0
	xch	r7			; result = 0
	ldm	8
	xch	r0			; 8 bits
.Land_loop:
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2			; a <<= 1 ; CY = a_bit (MSB)
	tcc				; ACC = a_bit, CY = 0
	xch	r1			; R1 = a_bit
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4			; b <<= 1 ; CY = b_bit
	tcc				; ACC = b_bit
	add	r1			; ACC = a_bit + b_bit (0..2)
	; and = bit1 of ACC: rotate right once -> bit1 goes to bit0
	rar				; ACC = s>>1 ; bit0 now = old bit1 = and
	; shift result left 8-bit, then add the and bit into bit0
	; (ACC currently holds and in bit0, plus possible high junk from rar of s;
	;  s<=2 so s>>1 <=1, ACC is exactly the and bit. CY = old s bit0 (xor) -- ignore.)
	xch	r1			; R1 = and bit
	clc
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6			; result <<= 1 (bit0 = 0)
	ld	r1
	rar				; CY <- and bit (ACC bit0 -> CY)
	jcn	nc, .Land_next		; and bit 0 -> leave result bit0 = 0
	ld	r7
	iac
	xch	r7			; set result bit0
.Land_next:
	isz	r0, .Land_loop
	ld	r6
	xch	r2
	ld	r7
	xch	r3			; result -> P1
	bbl	0

; ---------------------------------------------------------------------------
; __orqi3
; ---------------------------------------------------------------------------
	.section .text.__orqi3,"ax"
	.globl	__orqi3
__orqi3:
	ldm	0
	xch	r6
	ldm	0
	xch	r7
	ldm	8
	xch	r0
.Lor_loop:
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2			; a<<=1, CY=a_bit
	tcc
	xch	r1			; R1=a_bit
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4			; b<<=1, CY=b_bit
	tcc
	add	r1			; ACC = s (0..2) ; or = 1 iff s != 0
	xch	r1			; R1 = s
	clc
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6			; result <<= 1
	ld	r1
	jcn	z, .Lor_next		; s == 0 -> or bit 0
	ld	r7
	iac
	xch	r7			; set result bit0
.Lor_next:
	isz	r0, .Lor_loop
	ld	r6
	xch	r2
	ld	r7
	xch	r3
	bbl	0

; ---------------------------------------------------------------------------
; __xorqi3
; ---------------------------------------------------------------------------
	.section .text.__xorqi3,"ax"
	.globl	__xorqi3
__xorqi3:
	ldm	0
	xch	r6
	ldm	0
	xch	r7
	ldm	8
	xch	r0
.Lxor_loop:
	clc
	ld	r3
	ral
	xch	r3
	ld	r2
	ral
	xch	r2			; a<<=1, CY=a_bit
	tcc
	xch	r1			; R1=a_bit
	clc
	ld	r5
	ral
	xch	r5
	ld	r4
	ral
	xch	r4			; b<<=1, CY=b_bit
	tcc
	add	r1			; ACC = s ; xor = s bit0
	rar				; CY <- s bit0 (= xor) ; ACC = s>>1
	tcc				; ACC = xor bit, CY = 0
	xch	r1			; R1 = xor bit
	clc
	ld	r7
	ral
	xch	r7
	ld	r6
	ral
	xch	r6			; result <<= 1
	ld	r1
	jcn	z, .Lxor_next		; xor bit 0 -> leave result bit0 = 0
	ld	r7
	iac
	xch	r7			; set result bit0
.Lxor_next:
	isz	r0, .Lxor_loop
	ld	r6
	xch	r2
	ld	r7
	xch	r3
	bbl	0
