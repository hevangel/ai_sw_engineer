; libi4004rt -- software return/spill stack helpers (abi.md sec.4.4)
; ============================================================================
; The 4004 hardware return stack is only 3 deep. For a call graph deeper than
; 3, abi.md sec.4.4 defines a SOFTWARE stack in a dedicated 4002 region
; (platform/link.ld: 4002 #1, handles 0x40..0x7F), pointed at by the SSP in
; P7 (R14:R15), growing UPWARD. crt0 initialises SSP to __sstack_base.
;
; These helpers are the stack machinery the backend calls when it spills or
; implements a software-managed call:
;   __push_byte(u8 v=P1)        : RAM[SSP..SSP+1] <- v (lo nibble first); SSP+=2
;   __pop_byte() -> u8 (P1)     : SSP-=2; P1 <- RAM[SSP..SSP+1]
;   __push3(u8 lo=P1.lo, mid=P1.hi, hi=P2.lo) : push a 12-bit value as 3 nibbles
;                                 (the sec.4.4 return-address triple); SSP+=3
;   __pop3() -> P1.lo/P1.hi/P2.lo : SSP-=3; read the triple back
;
; SSP (P7 = R14:R15) is the 8-bit SRC address into 4002 #1; R14 = high nibble
; [chip:2][reg:2], R15 = low nibble [char:4]. The region is one 4002 chip
; (chip 1), so an address is 0x40..0x7F; the helpers advance R15 (char) and the
; chip/reg bits in R14 stay fixed for pushes within one 64-nibble region.
; A deep graph that overflows 64 nibbles is a documented limit (abi.md sec.7
; open Q4: the firmware never uses the soft stack; it is <=3 deep).
;
; SSP points at the NEXT FREE nibble (grows up): push writes at SSP then
; increments; pop decrements then reads. Leaf w.r.t. the hardware stack (these
; themselves are depth-1 JMS/BBL). P0 (R0:R1) is the transient SRC pointer.

; ---------------------------------------------------------------------------
; __push_byte : RAM[SSP]=v.lo ; RAM[SSP+1]=v.hi ; SSP+=2.  v in P1 (R2:R3).
; ---------------------------------------------------------------------------
	.section .text.__push_byte,"ax"
	.globl	__push_byte
__push_byte:
	; SRC = SSP (P7)
	src	p7
	ld	r3
	wrm				; RAM[SSP] = v.lo
	inc	r15			; SSP.char++
	src	p7
	ld	r2
	wrm				; RAM[SSP+1] = v.hi
	inc	r15			; SSP += 2 total
	bbl	0

; ---------------------------------------------------------------------------
; __pop_byte : SSP-=2 ; P1.lo = RAM[SSP] ; P1.hi = RAM[SSP+1].
; ---------------------------------------------------------------------------
	.section .text.__pop_byte,"ax"
	.globl	__pop_byte
__pop_byte:
	ld	r15
	dac
	xch	r15			; SSP.char--  (now points at v.hi)
	src	p7
	rdm
	xch	r2			; P1.hi = RAM[SSP+1]
	ld	r15
	dac
	xch	r15			; SSP.char--  (now points at v.lo)
	src	p7
	rdm
	xch	r3			; P1.lo = RAM[SSP]
	bbl	0

; ---------------------------------------------------------------------------
; __push3 : push the 12-bit value {P2.lo:P1.hi:P1.lo} as three nibbles, low
; first, at SSP; SSP+=3. This is the abi.md sec.4.4 return-address triple
; (offset-low, offset-high, page).
; ---------------------------------------------------------------------------
	.section .text.__push3,"ax"
	.globl	__push3
__push3:
	src	p7
	ld	r3
	wrm				; nibble0 = offset low (P1.lo)
	inc	r15
	src	p7
	ld	r2
	wrm				; nibble1 = offset high (P1.hi)
	inc	r15
	src	p7
	ld	r5
	wrm				; nibble2 = page (P2.lo)
	inc	r15
	bbl	0

; ---------------------------------------------------------------------------
; __pop3 : SSP-=3; read the triple back into P1.lo, P1.hi, P2.lo.
; ---------------------------------------------------------------------------
	.section .text.__pop3,"ax"
	.globl	__pop3
__pop3:
	ld	r15
	dac
	xch	r15
	src	p7
	rdm
	xch	r5			; P2.lo = page
	ld	r15
	dac
	xch	r15
	src	p7
	rdm
	xch	r2			; P1.hi = offset high
	ld	r15
	dac
	xch	r15
	src	p7
	rdm
	xch	r3			; P1.lo = offset low
	bbl	0
